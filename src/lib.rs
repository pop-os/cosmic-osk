// SPDX-License-Identifier: GPL-3.0-only

use calloop::channel;
use cosmic::{
    Application, Element,
    app::{Core, Settings, Task},
    cosmic_config::{self, CosmicConfigEntry},
    executor,
    iced::{
        Alignment, Length, Limits, Subscription,
        futures::{self, sink::SinkExt},
        platform_specific::{
            runtime::wayland::layer_surface::{IcedMargin, IcedOutput, SctkLayerSurfaceSettings},
            shell::wayland::commands::layer_surface::{
                Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
            },
        },
        stream,
        window::Id as WindowId,
    },
    style, widget,
};
use std::any::TypeId;

use config::{CONFIG_VERSION, Config};
pub mod config;

use layout::Layout;
pub mod layout;

pub mod localize;

use wayland::{VkEvent, vk_channels};
pub mod wayland;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    localize::localize();

    let (config_handler, config) = match cosmic_config::Config::new(App::APP_ID, CONFIG_VERSION) {
        Ok(config_handler) => {
            let config = Config::get_entry(&config_handler).unwrap_or_else(|(errs, config)| {
                log::info!("errors loading config: {:?}", errs);
                config
            });
            (Some(config_handler), config)
        }
        Err(err) => {
            log::error!("failed to create config handler: {}", err);
            (None, Config::default())
        }
    };

    let mut settings = Settings::default();
    settings = settings.theme(config.app_theme.theme());
    settings = settings.exit_on_close(false);
    settings = settings.transparent(true);
    settings = settings.no_main_window(true);

    let flags = Flags {
        config_handler,
        config,
    };
    cosmic::app::run::<App>(settings, flags)?;

    Ok(())
}

#[derive(Clone, Debug)]
pub struct Flags {
    config_handler: Option<cosmic_config::Config>,
    config: Config,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Message {
    Button {
        layer: usize,
        row: usize,
        col: usize,
    },
    Layout(Layout),
    VkeTx(channel::Sender<VkEvent>),
}

pub struct App {
    core: Core,
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    layout: Option<Layout>,
    layer: usize,
    surface_id: Option<WindowId>,
    vke_tx: Option<channel::Sender<VkEvent>>,
}

/// Implement [`cosmic::Application`] to integrate with COSMIC.
impl Application for App {
    /// Default async executor to use with the app.
    type Executor = executor::Default;

    /// Argument received [`cosmic::Application::new`].
    type Flags = Flags;

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "com.system76.CosmicEdit";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Creates the application, and optionally emits command on initialize.
    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let app = App {
            core,
            config_handler: flags.config_handler,
            config: flags.config,
            layer: 0,
            layout: None,
            surface_id: None,
            vke_tx: None,
        };

        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Button { layer, row, col } => {
                if let Some(layout) = &self.layout {
                    if let Some(key) = layout
                        .layers
                        .get(layer)
                        .and_then(|x| x.rows.get(row))
                        .and_then(|x| x.get(col))
                    {
                        log::warn!("{:?}", key);
                        match key.action {
                            layout::Action::None => {}
                            layout::Action::Keycode(kc) => {
                                match &self.vke_tx {
                                    Some(vke_tx) => {
                                        //TODO: run in task
                                        vke_tx.send(VkEvent::KeyPress(kc)).unwrap();
                                        vke_tx.send(VkEvent::KeyRelease(kc)).unwrap();
                                    }
                                    None => {
                                        log::warn!("no virtual keyboard event sender");
                                    }
                                }
                            }
                            layout::Action::Layer(layer) => {
                                if layer < layout.layers.len() {
                                    self.layer = layer;
                                } else {
                                    log::warn!("invalid layer {}", layer);
                                }
                            }
                        }
                    }
                }
            }
            Message::Layout(layout) => {
                let mut max_height = 0;
                for layer in layout.layers.iter() {
                    let mut height = 16; // 8 padding top and bottom
                    for (row, _) in layer.rows.iter().enumerate() {
                        if row > 0 {
                            height += 8; // 8 spacing
                        }
                        height += 64;
                    }
                    if height > max_height {
                        max_height = height;
                    }
                }

                self.layer = 0;
                self.layout = Some(layout);

                //TODO: destroy and recreate surface when layout changes?
                if !self.surface_id.is_some() {
                    let surface_id = WindowId::unique();
                    self.surface_id = Some(surface_id);
                    return get_layer_surface(SctkLayerSurfaceSettings {
                        id: surface_id,
                        layer: Layer::Top,
                        keyboard_interactivity: KeyboardInteractivity::None,
                        pointer_interactivity: true,
                        anchor: Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                        output: IcedOutput::Active,
                        namespace: "cosmic-osk".into(),
                        size: Some((None, Some(max_height))),
                        margin: IcedMargin {
                            top: 0,
                            bottom: 0,
                            left: 0,
                            right: 0,
                        },
                        exclusive_zone: max_height as i32,
                        size_limits: Limits::NONE.min_width(320.0).min_height(max_height as f32),
                    });
                }
            }
            Message::VkeTx(vke_tx) => {
                self.vke_tx = Some(vke_tx);
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<Message> {
        unimplemented!()
    }

    fn view_window(&self, id: WindowId) -> Element<Message> {
        let element: Element<_> = if let Some(layout_layer) = self
            .layout
            .as_ref()
            .and_then(|layout| layout.layers.get(self.layer))
        {
            let mut grid = widget::column::with_capacity(layout_layer.rows.len())
                .align_x(Alignment::Center)
                .spacing(8.0);
            for (row, layout_row) in layout_layer.rows.iter().enumerate() {
                let mut r = widget::row::with_capacity(layout_row.len())
                    .align_y(Alignment::Center)
                    .spacing(8.0);
                for (col, key) in layout_row.iter().enumerate() {
                    r = r.push(
                        widget::button::custom(
                            widget::container(widget::text(&key.name)).center(Length::Fill),
                        )
                        .height(Length::Fixed(64.0))
                        .width(Length::Fixed(64.0 * key.width))
                        .on_press(Message::Button {
                            layer: self.layer,
                            row,
                            col,
                        }),
                    );
                }
                grid = grid.push(r);
            }
            grid.into()
        } else {
            widget::text(format!("missing layout")).into()
        };
        widget::container(element)
            .class(style::Container::WindowBackground)
            .center(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        struct VkSubscription;
        Subscription::run_with_id(
            TypeId::of::<VkSubscription>(),
            stream::channel(100, |mut output| async move {
                //TODO: can this be made simpler?
                tokio::task::spawn_blocking(move || {
                    let (vke_tx, layout_rx) = vk_channels();
                    futures::executor::block_on(async {
                        output.send(Message::VkeTx(vke_tx)).await
                    })
                    .unwrap();
                    loop {
                        let layout = layout_rx.recv().unwrap();
                        futures::executor::block_on(async {
                            output.send(Message::Layout(layout)).await
                        })
                        .unwrap();
                    }
                })
                .await
                .unwrap()
            }),
        )
    }
}

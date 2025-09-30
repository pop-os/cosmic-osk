// SPDX-License-Identifier: GPL-3.0-only

use calloop::channel;
use cosmic::{
    Action, Application, Element,
    app::{Core, Settings, Task},
    cosmic_config::{self, CosmicConfigEntry},
    executor,
    iced::{
        Length, Limits, Subscription,
        futures::{self, sink::SinkExt},
        platform_specific::{
            runtime::wayland::layer_surface::{IcedMargin, IcedOutput, SctkLayerSurfaceSettings},
            shell::wayland::commands::layer_surface::{
                Anchor, KeyboardInteractivity, Layer, get_layer_surface,
            },
        },
        stream,
        window::Id as WindowId,
    },
    iced_winit::commands::layer_surface::destroy_layer_surface,
    style, widget,
};
use std::any::TypeId;

use config::{CONFIG_VERSION, Config};
pub mod config;

use layout::Layout;
pub mod layout;

pub mod localize;

use wayland::{VkEvent, vk_channels};

use crate::wayland::KeyModifiers;
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
    Key {
        action: layout::Action,
        pressed: bool,
    },
    Layout(Layout),
    Modifier(KeyModifiers),
    VkeTx(channel::Sender<VkEvent>),
    ChangeLayoutSize,
    ToggleLayout,
}

pub struct App {
    core: Core,
    _config_handler: Option<cosmic_config::Config>,
    _config: Config,
    key_padding: usize,
    key_size: usize,
    layout: Option<Layout>,
    layer: usize,
    surface_id: Option<WindowId>,
    vke_tx: Option<channel::Sender<VkEvent>>,
    is_full_layout: bool,
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
            _config_handler: flags.config_handler,
            _config: flags.config,
            key_padding: 2,
            key_size: 64,
            layer: 0,
            layout: None,
            surface_id: None,
            vke_tx: None,
            is_full_layout: false,
        };

        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Key { action, pressed } => {
                match action {
                    layout::Action::None => {}
                    layout::Action::Keycode(kc) => {
                        match &self.vke_tx {
                            Some(vke_tx) => {
                                //TODO: run in task
                                vke_tx.send(VkEvent::Key(kc, pressed)).unwrap();
                            }
                            None => {
                                log::warn!("no virtual keyboard event sender");
                            }
                        }
                    }
                    layout::Action::ToggleLayout => {
                        if pressed {
                            return Task::done(Action::App(Message::ToggleLayout));
                        }
                    }
                }
            }
            Message::ToggleLayout => {
                self.is_full_layout = !self.is_full_layout;
                return Task::done(Action::App(Message::ChangeLayoutSize));
            }
            Message::ChangeLayoutSize => {
                let mut height = 0;
                let Some(layout) = &self.layout else {
                    return Task::none();
                };
                let layers = match self.is_full_layout {
                    true => &layout.full_layers,
                    false => &layout.partial_layers,
                };
                for layer in layers.iter() {
                    height = height.max((self.key_size + self.key_padding * 2) * layer.rows.len());
                }
                let mut destroy_task = Task::none();
                if let Some(id) = self.surface_id {
                    destroy_task = destroy_layer_surface(id);
                }
                let surface_id = WindowId::unique();
                self.surface_id = Some(surface_id);
                let create_task = get_layer_surface(SctkLayerSurfaceSettings {
                    id: surface_id,
                    layer: Layer::Top,
                    keyboard_interactivity: KeyboardInteractivity::None,
                    pointer_interactivity: true,
                    anchor: Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                    output: IcedOutput::Active,
                    namespace: "cosmic-osk".into(),
                    size: Some((None, Some(height as u32))),
                    margin: IcedMargin {
                        top: 0,
                        bottom: 0,
                        left: 0,
                        right: 0,
                    },
                    exclusive_zone: height as i32,
                    size_limits: Limits::NONE.min_width(320.0).min_height(height as f32),
                });

                return destroy_task.chain(create_task);
            }
            Message::Layout(layout) => {
                self.layout = Some(layout);
                return Task::done(Action::App(Message::ChangeLayoutSize));
            }
            Message::Modifier(modifier) => {
                // TODO: implement layers properly
                // WARN: capslock does not behave like shift for symbols...
                let shift = (modifier.shift as u32) ^ (modifier.capslock as u32);
                self.layer = shift as usize;
            }
            Message::VkeTx(vke_tx) => {
                self.vke_tx = Some(vke_tx);
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        unimplemented!()
    }

    fn view_window(&self, _id: WindowId) -> Element<'_, Message> {
        let element = self.create_keyboard();
        widget::container(element)
            .class(style::Container::Background)
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
                    let (vke_tx, msg_rx) = vk_channels();
                    futures::executor::block_on(async {
                        output.send(Message::VkeTx(vke_tx)).await
                    })
                    .unwrap();
                    loop {
                        let msg = msg_rx.recv().unwrap();
                        futures::executor::block_on(async { output.send(msg).await }).unwrap();
                    }
                })
                .await
                .unwrap()
            }),
        )
    }
}

impl App {
    fn create_keyboard(&self) -> Element<'_, Message> {
        let Some(layout) = self.layout.as_ref() else {
            return widget::text(format!("missing layout")).into();
        };
        let layers = match self.is_full_layout {
            true => &layout.full_layers,
            false => &layout.partial_layers,
        };
        let Some(layout_layer) = layers.get(self.layer) else {
            return widget::text(format!("missing layer")).into();
        };

        let mut grid = widget::column::with_capacity(layout_layer.rows.len());
        for layout_row in layout_layer.rows.iter() {
            let mut r = widget::row::with_capacity(layout_row.len());
            for key in layout_row.iter() {
                r = r.push(
                    widget::container(
                        widget::button::custom(
                            widget::container(widget::text(&key.name)).center(Length::Fill),
                        )
                        //TODO: use custom style?
                        .class(style::Button::MenuItem)
                        //WARN: causes sticky buttons when typing "wrong"
                        .on_press_down(Message::Key {
                            action: key.action,
                            pressed: true,
                        })
                        .on_press(Message::Key {
                            action: key.action,
                            pressed: false,
                        }),
                    )
                    .padding(self.key_padding as u16)
                    .height(Length::Fixed(self.key_size as f32))
                    .width(Length::FillPortion(key.width as u16)),
                );
            }
            grid = grid.push(r);
        }
        grid = grid.max_width(1200.0);
        grid.into()
    }
}

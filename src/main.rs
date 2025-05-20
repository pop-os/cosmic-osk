// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    Application, Element,
    app::{Core, Settings, Task},
    cosmic_config::{self, CosmicConfigEntry},
    executor,
    iced::{
        Alignment, Length, Limits,
        platform_specific::{
            runtime::wayland::layer_surface::{IcedMargin, IcedOutput, SctkLayerSurfaceSettings},
            shell::wayland::commands::layer_surface::{
                Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
            },
        },
        window::Id as WindowId,
    },
    style, widget,
};

use config::{CONFIG_VERSION, Config};
mod config;

use layout::Layout;
mod layout;

mod localize;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
}

pub struct App {
    core: Core,
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    layout: Layout,
    layer: usize,
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
            layout: Layout::us(),
            layer: 0,
        };

        let mut max_height = 0;
        for layer in app.layout.layers.iter() {
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

        (
            app,
            get_layer_surface(SctkLayerSurfaceSettings {
                id: WindowId::unique(),
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
            }),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Button { layer, row, col } => {
                if let Some(key) = self
                    .layout
                    .layers
                    .get(layer)
                    .and_then(|x| x.rows.get(row))
                    .and_then(|x| x.get(col))
                {
                    log::warn!("TODO: {:?}", key);
                    match key.action {
                        layout::Action::Character => {
                            //TODO
                        }
                        layout::Action::Layer(layer) => {
                            if layer < self.layout.layers.len() {
                                self.layer = layer;
                            } else {
                                log::warn!("invalid layer {}", layer);
                            }
                        }
                    }
                }
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<Message> {
        unimplemented!()
    }

    fn view_window(&self, id: WindowId) -> Element<Message> {
        let element: Element<_> = if let Some(layout_layer) = self.layout.layers.get(self.layer) {
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
            widget::text(format!("layer 0 not found in layout")).into()
        };
        widget::container(element)
            .class(style::Container::WindowBackground)
            .center(Length::Fill)
            .into()
    }
}

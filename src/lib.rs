// SPDX-License-Identifier: GPL-3.0-only

use xkbcommon::xkb;

use cosmic::{
    Application, Element,
    app::{Core, Settings, Task},
    cosmic_config::{self, CosmicConfigEntry},
    executor,
    iced::{
        Length, Limits, Subscription,
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
pub mod config;

mod ei;

use layout::Layout;
pub mod layout;

pub mod localize;

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
    Layer(usize),
    Ei(ei::Msg),
}

pub struct App {
    core: Core,
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    key_padding: usize,
    key_size: usize,
    layout: Option<Layout>,
    layer: usize,
    surface_id: Option<WindowId>,
    // TODO reis state
    ei_conn: Option<reis::event::Connection>,
    ei_keyboard: Option<(reis::ei::Device, reis::ei::Keyboard)>,
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
            key_padding: 4,
            key_size: 64,
            layer: 0,
            layout: None,
            surface_id: None,
            ei_conn: None,
            ei_keyboard: None,
        };

        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Key { action, pressed } => {
                match action {
                    layout::Action::None => {}
                    layout::Action::Keycode(kc) => {
                        // TODO send key to reis
                        if let Some((device, keyboard)) = &self.ei_keyboard {
                            let kc = u32::from(kc) - 8;
                            let state = if pressed {
                                reis::ei::keyboard::KeyState::Press
                            } else {
                                reis::ei::keyboard::KeyState::Released
                            };
                            keyboard.key(kc, state);
                            // TODO device frame
                            device.frame(0, 1); // TODO
                            self.ei_conn.as_ref().unwrap().flush();
                        }
                    }
                }
            }
            Message::Layer(layer) => {
                self.layer = layer;
            }
            Message::Ei(evt) => {
                match evt {
                    // TODO handle modifiers
                    ei::Msg::Connection(conn) => {
                        self.ei_conn = Some(conn);
                    }
                    ei::Msg::Event(reis::event::EiEvent::SeatAdded(evt)) => {
                        use reis::event::DeviceCapability;
                        evt.seat
                            .bind_capabilities(DeviceCapability::Keyboard.into());
                        let _ = self.ei_conn.as_ref().unwrap().flush();
                    }
                    ei::Msg::Event(reis::event::EiEvent::DeviceAdded(evt)) => {
                        self.ei_keyboard = Some((
                            evt.device.device().clone(),
                            evt.device.interface::<reis::ei::Keyboard>().unwrap(),
                        ));
                        let serial = self.ei_conn.as_ref().unwrap().serial();
                        evt.device.device().start_emulating(0, serial);
                        let keymap = evt.device.keymap().unwrap();
                        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
                        let layout = Layout::from(&unsafe {
                            xkb::Keymap::new_from_fd(
                                &ctx,
                                keymap.fd.try_clone().unwrap(),
                                keymap.size as usize,
                                xkb::KEYMAP_FORMAT_TEXT_V1,
                                xkb::KEYMAP_COMPILE_NO_FLAGS,
                            )
                            .unwrap()
                            .unwrap()
                        });

                        let mut height = 0;
                        for layer in layout.layers.iter() {
                            height = height
                                .max((self.key_size + self.key_padding * 2) * layer.rows.len());
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
                                size: Some((None, Some(height as u32))),
                                margin: IcedMargin {
                                    top: 0,
                                    bottom: 0,
                                    left: 0,
                                    right: 0,
                                },
                                exclusive_zone: height as i32,
                                size_limits: Limits::NONE
                                    .min_width(320.0)
                                    .min_height(height as f32),
                            });
                        }
                    }
                    _ => {}
                }
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
                        .width(Length::Fixed(self.key_size as f32 * key.width)),
                    );
                }
                grid = grid.push(r);
            }
            grid.into()
        } else {
            widget::text(format!("missing layout")).into()
        };
        widget::container(element)
            .class(style::Container::Background)
            .center(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        ei::subscription().map(Message::Ei)
    }
}

// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    Application, Element,
    app::{Core, Settings, Task},
    cosmic_config::{self, CosmicConfigEntry},
    cosmic_theme, executor,
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
    theme, widget,
};
use reis::ei::keyboard::KeyState;
use std::{collections::HashSet, process};
use xkbcommon::xkb;

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
        kind: layout::KeyKind,
        keycode: layout::KeyCode,
        pressed: bool,
    },
    Quit,
    Ei(ei::Msg),
}

pub struct App {
    core: Core,
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    key_padding: usize,
    key_size: usize,
    layouts: Option<Vec<Layout>>,
    group: u32,
    layer: usize,
    sticky: HashSet<layout::KeyCode>,
    surface_id: Option<WindowId>,
    xkb_state: Option<xkb::State>,
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
            layouts: None,
            group: 0,
            sticky: HashSet::new(),
            surface_id: None,
            xkb_state: None,
            ei_conn: None,
            ei_keyboard: None,
        };

        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Key {
                kind,
                keycode,
                pressed,
            } => {
                let Some(xkb_state) = &mut self.xkb_state else {
                    return Task::none();
                };
                // TODO send key to reis
                if let Some((device, keyboard)) = &self.ei_keyboard {
                    let (state, release_mods) = match kind {
                        layout::KeyKind::Mod {
                            name: mod_name,
                            sticky,
                        } if sticky => {
                            // Sticky modifiers toggle, so ignore button release
                            if !pressed {
                                return Task::none();
                            }

                            (
                                if self.sticky.remove(&keycode) {
                                    // If the modifier is already stored, we need to release it
                                    KeyState::Released
                                } else {
                                    // If the modifier is not stored, store it and press it
                                    self.sticky.insert(keycode);
                                    KeyState::Press
                                },
                                false,
                            )
                        }
                        _ => (
                            if pressed {
                                KeyState::Press
                            } else {
                                KeyState::Released
                            },
                            true,
                        ),
                    };

                    let mut key = |keycode: layout::KeyCode, state: KeyState| {
                        xkb_state.update_key(
                            keycode.xkb(),
                            match state {
                                KeyState::Press => xkb::KeyDirection::Down,
                                KeyState::Released => xkb::KeyDirection::Up,
                            },
                        );
                        keyboard.key(keycode.evdev(), state);
                    };

                    key(keycode, state);

                    if release_mods {
                        // Release non-permanent modifier keys
                        for kc in self.sticky.drain() {
                            key(kc, KeyState::Released);
                        }
                    }

                    // TODO device frame
                    device.frame(0, 1); // TODO
                    self.ei_conn
                        .as_ref()
                        .unwrap()
                        .flush()
                        .expect("failed to flush EI connection");
                }

                //TODO: use xkb::State::key_get_level
                let shift =
                    xkb_state.mod_name_is_active(xkb::MOD_NAME_SHIFT, xkb::STATE_MODS_EFFECTIVE);
                let caps =
                    xkb_state.mod_name_is_active(xkb::MOD_NAME_CAPS, xkb::STATE_MODS_EFFECTIVE);
                self.layer = if shift != caps { 1 } else { 0 };
            }
            Message::Quit => {
                process::exit(0);
            }
            Message::Ei(evt) => {
                match evt {
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
                        let xkb_keymap = unsafe {
                            xkb::Keymap::new_from_fd(
                                &ctx,
                                keymap.fd.try_clone().unwrap(),
                                keymap.size as usize,
                                xkb::KEYMAP_FORMAT_TEXT_V1,
                                xkb::KEYMAP_COMPILE_NO_FLAGS,
                            )
                            .unwrap()
                            .unwrap()
                        };
                        let layouts =
                            Layout::all(&xkb_keymap).unwrap_or_else(|| vec![Layout::default()]);

                        let mut height = 0;
                        for layout in &layouts {
                            for layer in layout.layers.iter() {
                                height = height
                                    .max((self.key_size + self.key_padding * 2) * layer.rows.len());
                            }
                        }

                        self.layer = 0;
                        self.layouts = Some(layouts);
                        self.xkb_state = Some(xkb::State::new(&xkb_keymap));

                        //TODO: destroy and recreate surface when layout changes?
                        if !self.surface_id.is_some() {
                            let surface_id = WindowId::unique();
                            self.surface_id = Some(surface_id);
                            return get_layer_surface(SctkLayerSurfaceSettings {
                                id: surface_id,
                                layer: Layer::Top,
                                keyboard_interactivity: KeyboardInteractivity::None,
                                input_zone: None,
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
                    // TODO handle other modifiers
                    ei::Msg::Event(reis::event::EiEvent::KeyboardModifiers(evt)) => {
                        self.group = evt.group;
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
        let cosmic_theme::Spacing {
            space_s,
            space_xs,
            space_xxs,
            ..
        } = theme::spacing();

        let element: Element<_> = if let Some(layout_layer) = self
            .layouts
            .as_ref()
            .and_then(|layouts| layouts.get(self.group as usize)?.layers.get(self.layer))
        {
            let mut grid = widget::column::with_capacity(layout_layer.rows.len() + 1);
            grid = grid.push(widget::row::with_children(vec![
                widget::space().width(Length::Fill).into(),
                widget::button::icon(widget::icon::from_name("window-close-symbolic"))
                    .on_press(Message::Quit)
                    .into(),
            ]));
            for layout_row in layout_layer.rows.iter() {
                let mut r = widget::row::with_capacity(layout_row.len() + 2);
                r = r.push(widget::space().width(Length::Fill));
                for key in layout_row.iter() {
                    let mut selected = false;
                    if let Some(kc) = key.keycode {
                        if let layout::KeyKind::Mod { name, sticky } = key.kind {
                            if sticky {
                                if self.sticky.contains(&kc) {
                                    selected = true;
                                }
                            } else {
                                if let Some(xkb_state) = &self.xkb_state {
                                    if xkb_state.mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE)
                                    {
                                        selected = true;
                                    }
                                }
                            }
                        }
                    }
                    //TODO: adjust to match design
                    let style = {
                        use widget::button::Catalog;
                        let adjust = move |theme: &cosmic::Theme,
                                           mut style: widget::button::Style|
                              -> widget::button::Style {
                            let cosmic = theme.cosmic();
                            if selected {
                                style.overlay = Some(cosmic::iced::Background::Color(
                                    cosmic.button.selected_state_color().into(),
                                ));
                                style.text_color = Some(cosmic.accent_text_color().into());
                                style.icon_color = style.text_color;
                            }
                            style.border_radius = cosmic.radius_s().into();
                            style
                        };
                        theme::Button::Custom {
                            active: Box::new(move |focused, theme| {
                                adjust(
                                    theme,
                                    theme.active(focused, selected, &theme::Button::MenuItem),
                                )
                            }),
                            disabled: Box::new(move |theme| {
                                adjust(theme, theme.disabled(&theme::Button::MenuItem))
                            }),
                            hovered: Box::new(move |focused, theme| {
                                adjust(
                                    theme,
                                    theme.hovered(focused, selected, &theme::Button::MenuItem),
                                )
                            }),
                            pressed: Box::new(move |focused, theme| {
                                adjust(
                                    theme,
                                    theme.pressed(focused, selected, &theme::Button::MenuItem),
                                )
                            }),
                        }
                    };
                    let mut button = widget::button::custom(
                        widget::container(if selected {
                            widget::text::heading(&key.name)
                        } else {
                            widget::text::body(&key.name)
                        })
                        .center(Length::Fill),
                    )
                    .class(style)
                    .selected(selected)
                    .width(Length::Fill)
                    .height(Length::Fill);
                    if let Some(keycode) = key.keycode {
                        button = button
                            .on_press_down(Message::Key {
                                kind: key.kind,
                                keycode,
                                pressed: true,
                            })
                            .on_press(Message::Key {
                                kind: key.kind,
                                keycode,
                                pressed: false,
                            });
                    }
                    r = r.push(
                        widget::container(button)
                            .padding(self.key_padding as u16)
                            .height(Length::Fixed(self.key_size as f32))
                            .width(Length::Fixed(self.key_size as f32 * key.width)),
                    );
                }
                r = r.push(widget::space().width(Length::Fill));
                grid = grid.push(r);
            }
            grid.into()
        } else {
            widget::text(format!("missing layout")).into()
        };
        widget::container(element)
            .padding([space_xxs, space_s, space_xs, space_s])
            .class(theme::Container::Background)
            .center(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        ei::subscription().map(Message::Ei)
    }
}

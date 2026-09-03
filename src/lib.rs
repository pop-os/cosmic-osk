// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    Application, Element,
    app::{Core, Settings, Task},
    cosmic_config::{self, CosmicConfigEntry},
    cosmic_theme, executor,
    iced::{
        Alignment, Length, Limits, Padding, Point, Rectangle, Size, Subscription, Vector, event,
        futures::{self, SinkExt},
        mouse,
        platform_specific::{
            runtime::wayland::layer_surface::{IcedMargin, IcedOutput, SctkLayerSurfaceSettings},
            shell::{
                commands::{blur::blur, layer_surface::set_padding},
                wayland::commands::layer_surface::{
                    Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
                    set_input_zone,
                },
            },
        },
        stream, window,
    },
    theme, widget,
};
use reis::ei::keyboard::KeyState;
use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    process,
};
use xkbcommon::xkb;

use config::{CONFIG_VERSION, Config};
pub mod config;

mod ei;

use layout::Layout;
pub mod layout;

pub mod localize;

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

#[derive(Clone, Copy, Debug)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GamepadAxisDirection {
    #[default]
    Center,
    Negative,
    Positive,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Message {
    Dock(bool),
    DragStart,
    DragMove(Point),
    DragEnd,
    Focus(widget::Id),
    Hide,
    Key {
        kind: layout::KeyKind,
        keycode: layout::KeyCode,
        pressed: bool,
    },
    Quit,
    SeatImActive {
        seat_id: u32,
        active: bool,
    },
    Size(Size),
    Ei(ei::Msg),
    Gilrs(gilrs::Event),
}

#[derive(Default)]
pub struct DragState {
    dragging: bool,
    start_pos: Option<Point>,
    mouse_pos: Option<Point>,
    surface_rect: Rectangle,
}

impl DragState {
    fn vector(&self) -> Option<Vector> {
        let start_pos = self.start_pos?;
        let mouse_pos = self.mouse_pos?;
        Some(mouse_pos - start_pos)
    }
}

pub struct App {
    core: Core,
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    docked: bool,
    drag: DragState,
    focus: Option<widget::Id>,
    ignore_activate: bool,
    key_padding: usize,
    key_size: usize,
    layouts: Option<Vec<Layout>>,
    group: u32,
    layer: usize,
    pressed: HashSet<layout::KeyCode>,
    sticky: HashSet<layout::KeyCode>,
    size: Size,
    surface_center: bool,
    surface_id: Option<window::Id>,
    surface_rect: Rectangle,
    xkb_state: Option<xkb::State>,
    // TODO reis state
    ei_conn: Option<reis::event::Connection>,
    ei_keyboard: Option<(reis::ei::Device, reis::ei::Keyboard)>,
    gamepad_axes: HashMap<gilrs::Axis, GamepadAxisDirection>,
    gamepad_shown: bool,
}

impl App {
    pub fn hide(&mut self) -> Task<Message> {
        if let Some(surface_id) = self.surface_id.take() {
            destroy_layer_surface(surface_id)
        } else {
            Task::none()
        }
    }

    pub fn show(&mut self) -> Task<Message> {
        if self.surface_id.is_some() {
            return Task::none();
        }

        self.surface_center = true;
        self.surface_rect = Default::default();
        if let Some(layouts) = &self.layouts {
            for layout in layouts.iter() {
                for layer in layout.layers.iter() {
                    let layer_height = (self.key_size + self.key_padding * 2) * layer.rows.len();
                    self.surface_rect.height = self.surface_rect.height.max(layer_height as f32);
                    for row in layer.rows.iter() {
                        let mut row_width = 0.0;
                        for key in row.iter() {
                            row_width +=
                                key.width * (self.key_size as f32) + self.key_padding as f32;
                        }
                        self.surface_rect.width = self.surface_rect.width.max(row_width);
                    }
                }
            }
        }

        let surface_id = window::Id::unique();
        self.surface_id = Some(surface_id);

        let mut settings = SctkLayerSurfaceSettings {
            id: surface_id,
            layer: Layer::Top,
            keyboard_interactivity: KeyboardInteractivity::None,
            input_zone: None,
            anchor: Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            output: IcedOutput::Active,
            namespace: "cosmic-osk".into(),
            size: Some((None, Some(self.surface_rect.height as u32))),
            margin: IcedMargin {
                top: 0,
                bottom: 0,
                left: 0,
                right: 0,
            },
            exclusive_zone: self.surface_rect.height as i32,
            size_limits: Limits::NONE
                .min_width(self.surface_rect.width)
                .min_height(self.surface_rect.height),
        };

        // Adjustments for floating mode
        if !self.docked {
            settings.anchor |= Anchor::TOP;
            //TODO: center by default
            settings.input_zone = Some(vec![self.surface_rect]);
            settings.size = None;
            settings.exclusive_zone = 0;
        }

        get_layer_surface(settings)
    }

    pub fn layout_layer(&self) -> Option<&layout::Layer> {
        self.layouts
            .as_ref()
            .and_then(|layouts| layouts.get(self.group as usize)?.layers.get(self.layer))
    }

    pub fn focus_index(&self) -> (usize, usize) {
        if let Some(layout_layer) = self.layout_layer() {
            for (y, layout_row) in layout_layer.rows.iter().enumerate() {
                for (x, key) in layout_row.iter().enumerate() {
                    if Some(&key.id) == self.focus.as_ref() {
                        return (x, y);
                    }
                }
            }
        }

        (0, 0)
    }

    pub fn find_focus(&self) -> Option<((usize, usize), Rectangle, layout::Key)> {
        if let Some(layout_layer) = self.layout_layer() {
            for (row_i, row) in layout_layer.rows.iter().enumerate() {
                let mut x = 0.0;
                for (col_i, key) in row.iter().enumerate() {
                    if Some(&key.id) == self.focus.as_ref() {
                        return Some((
                            (row_i, col_i),
                            Rectangle {
                                x,
                                y: row_i as f32,
                                width: key.width,
                                height: 1.0,
                            },
                            key.clone(),
                        ));
                    }
                    x += key.width;
                }
            }
        }

        None
    }

    pub fn move_focus(&mut self, dir: FocusDirection) -> Task<Message> {
        let (mut index, rect) = match self.find_focus() {
            Some((index, rect, _)) => (index, rect),
            //TODO: default to middle of layout?
            None => ((0, 0), Rectangle::default()),
        };

        let mut focus = None;
        if let Some(layout_layer) = self.layout_layer() {
            if let Some(row) = layout_layer.rows.get(index.0) {
                match dir {
                    FocusDirection::Left => {
                        if index.1 == 0 {
                            index.1 = row.len();
                        }
                        index.1 = index.1.saturating_sub(1);
                    }
                    FocusDirection::Right => {
                        index.1 = index.1.saturating_add(1);
                        if index.1 >= row.len() {
                            index.1 = 0;
                        }
                    }
                    FocusDirection::Up | FocusDirection::Down => {
                        let row_i = if matches!(dir, FocusDirection::Up) {
                            index.0.saturating_sub(1)
                        } else {
                            index.0.saturating_add(1)
                        };
                        if let Some(next_row) = layout_layer.rows.get(row_i) {
                            let mut max_col_i = None;
                            let mut max_overlap = 0.0;
                            let mut x = 0.0;
                            for (col_i, key) in next_row.iter().enumerate() {
                                let next_x = x + key.width;
                                let max_left = x.max(rect.x);
                                let min_right = next_x.min(rect.x + rect.width);
                                let overlap = (min_right - max_left).max(0.0);
                                if overlap > max_overlap {
                                    max_col_i = Some(col_i);
                                    max_overlap = overlap;
                                }
                                x = next_x;
                            }
                            if let Some(col_i) = max_col_i {
                                if let Some(key) = next_row.get(col_i) {
                                    focus = Some(key.id.clone());
                                }
                            }
                        }
                    }
                }

                if focus.is_none() {
                    if let Some(key) = row.get(index.1) {
                        focus = Some(key.id.clone());
                    }
                }
            }
        }

        if let Some(id) = focus {
            self.focus = Some(id.clone());
            widget::button::focus(id)
        } else {
            Task::none()
        }
    }
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
    const APP_ID: &'static str = "com.system76.CosmicOSK";

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
            docked: true,
            drag: DragState::default(),
            focus: None,
            ignore_activate: false,
            key_padding: 4,
            key_size: 64,
            layer: 0,
            layouts: None,
            group: 0,
            pressed: HashSet::new(),
            sticky: HashSet::new(),
            size: Size::default(),
            surface_center: false,
            surface_id: None,
            surface_rect: Rectangle::default(),
            xkb_state: None,
            ei_conn: None,
            ei_keyboard: None,
            gamepad_axes: HashMap::new(),
            gamepad_shown: false,
        };

        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Dock(dock) => {
                if dock != self.docked {
                    let hide_task = self.hide();
                    self.docked = dock;
                    let show_task = self.show();
                    return Task::batch([hide_task, show_task]);
                }
            }
            Message::DragStart => {
                if !self.docked && !self.drag.dragging {
                    self.drag = DragState::default();
                    self.drag.dragging = true;
                    self.drag.surface_rect = self.surface_rect;
                }
            }
            Message::DragMove(point) => {
                if !self.docked && self.drag.dragging {
                    self.drag.mouse_pos = Some(point);
                    if let Some(vector) = self.drag.vector() {
                        self.drag.surface_rect = self.surface_rect + vector;
                        // Clamp to display
                        self.drag.surface_rect.x = self
                            .drag
                            .surface_rect
                            .x
                            .min(self.size.width - self.drag.surface_rect.width)
                            .max(0.0);
                        self.drag.surface_rect.y = self
                            .drag
                            .surface_rect
                            .y
                            .min(self.size.height - self.drag.surface_rect.height)
                            .max(0.0);
                    } else {
                        self.drag.start_pos = self.drag.mouse_pos;
                    }
                    if let Some(surface_id) = self.surface_id {
                        return Task::batch(vec![
                            set_padding(
                                surface_id,
                                IcedMargin {
                                    left: self.drag.surface_rect.x.max(0.) as i32,
                                    top: self.drag.surface_rect.y.max(0.) as i32,
                                    bottom: (self.size.height
                                        - self.drag.surface_rect.y
                                        - self.drag.surface_rect.height)
                                        .max(0.) as i32,
                                    right: (self.size.width
                                        - self.drag.surface_rect.x
                                        - self.drag.surface_rect.width)
                                        .max(0.) as i32,
                                },
                            ),
                            blur(surface_id, Some(vec![self.drag.surface_rect])).discard(),
                        ]);
                    }
                }
            }
            Message::DragEnd => {
                if !self.docked && self.drag.dragging {
                    self.surface_rect = self.drag.surface_rect;
                    self.drag = DragState::default();
                    if let Some(surface_id) = self.surface_id {
                        return set_input_zone(surface_id, Some(vec![self.surface_rect]));
                    }
                }
            }
            Message::Focus(id) => {
                self.focus = Some(id.clone());
                return widget::button::focus(id);
            }
            Message::Hide => {
                self.ignore_activate = true;
                return self.hide();
            }
            Message::Key {
                kind,
                keycode,
                mut pressed,
            } => {
                let Some(xkb_state) = &mut self.xkb_state else {
                    return Task::none();
                };
                // TODO send key to reis
                if let Some((device, keyboard)) = &self.ei_keyboard {
                    let release_mods = match kind {
                        layout::KeyKind::Mod { sticky, .. } if sticky => {
                            // Sticky modifiers toggle, so ignore button release
                            if !pressed {
                                return Task::none();
                            }

                            // If the modifier is already stored, we need to release it
                            pressed = !self.sticky.remove(&keycode);
                            if pressed {
                                self.sticky.insert(keycode);
                            }
                            false
                        }
                        _ => true,
                    };

                    let mut key = |keycode: layout::KeyCode, pressed: bool| {
                        if pressed {
                            self.pressed.insert(keycode);
                        } else {
                            self.pressed.remove(&keycode);
                        }
                        xkb_state.update_key(
                            keycode.xkb(),
                            if pressed {
                                xkb::KeyDirection::Down
                            } else {
                                xkb::KeyDirection::Up
                            },
                        );
                        keyboard.key(
                            keycode.evdev(),
                            if pressed {
                                KeyState::Press
                            } else {
                                KeyState::Released
                            },
                        );
                    };

                    key(keycode, pressed);

                    if release_mods {
                        // Release non-permanent modifier keys
                        for kc in self.sticky.drain() {
                            key(kc, false);
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
            Message::SeatImActive { seat_id, active } => {
                eprintln!("{} active: {}", seat_id, active);
                if active {
                    if !self.ignore_activate {
                        return self.show();
                    }
                } else {
                    self.ignore_activate = false;
                }
            }
            Message::Size(size) => {
                eprintln!("size: {:?}", size);
                let mut tasks = Vec::new();
                self.size = size;
                if self.surface_center {
                    self.surface_center = false;
                    self.surface_rect.x = (size.width - self.surface_rect.width) / 2.0;
                    self.surface_rect.y = (size.height - self.surface_rect.height) / 2.0;
                    if let Some(surface_id) = self.surface_id {
                        tasks.push(set_input_zone(surface_id, Some(vec![self.surface_rect])));
                    }
                }
                if let Some(surface_id) = self.surface_id
                    && !self.docked
                {
                    tasks.push(set_padding(
                        surface_id,
                        IcedMargin {
                            left: self.surface_rect.x.max(0.) as i32,
                            top: self.surface_rect.y.max(0.) as i32,
                            bottom: (self.size.height
                                - self.surface_rect.y
                                - self.surface_rect.height)
                                .max(0.) as i32,
                            right: (self.size.width - self.surface_rect.x - self.surface_rect.width)
                                .max(0.) as i32,
                        },
                    ));
                    tasks.push(blur(surface_id, Some(vec![self.surface_rect])).discard());
                }
                return Task::batch(tasks);
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

                        self.layer = 0;
                        self.layouts = Some(layouts);
                        self.xkb_state = Some(xkb::State::new(&xkb_keymap));

                        //TODO: destroy and recreate surface when layout changes?
                        return self.show();
                    }
                    // TODO handle other modifiers
                    ei::Msg::Event(reis::event::EiEvent::KeyboardModifiers(evt)) => {
                        self.group = evt.group;
                    }
                    _ => {}
                }
            }
            Message::Gilrs(event) => {
                use gilrs::{Axis, Button, EventType};

                // Show the gamepad mappings after any gamepad event
                self.gamepad_shown = true;

                match event.event {
                    EventType::AxisChanged(axis, value, _) => {
                        // Emulate a dpad press on axis movement
                        const AXIS_OFF: f32 = 0.25;
                        const AXIS_ON: f32 = 0.5;
                        let last_dir = self.gamepad_axes.get(&axis).copied().unwrap_or_default();
                        let dir = if value < -AXIS_ON {
                            GamepadAxisDirection::Negative
                        } else if value > AXIS_ON {
                            GamepadAxisDirection::Positive
                        } else if value > -AXIS_OFF && value < AXIS_OFF {
                            GamepadAxisDirection::Center
                        } else {
                            last_dir
                        };
                        if last_dir != dir {
                            eprintln!("{:?}: {:?}", axis, dir);
                            self.gamepad_axes.insert(axis, dir);
                            match axis {
                                Axis::LeftStickX | Axis::RightStickX => match dir {
                                    GamepadAxisDirection::Negative => {
                                        return self.move_focus(FocusDirection::Left);
                                    }
                                    GamepadAxisDirection::Positive => {
                                        return self.move_focus(FocusDirection::Right);
                                    }
                                    _ => {}
                                },
                                Axis::LeftStickY | Axis::RightStickY => match dir {
                                    GamepadAxisDirection::Negative => {
                                        return self.move_focus(FocusDirection::Down);
                                    }
                                    GamepadAxisDirection::Positive => {
                                        return self.move_focus(FocusDirection::Up);
                                    }
                                    _ => {}
                                },
                                _ => {}
                            }
                        }
                    }
                    EventType::ButtonPressed(button, _) | EventType::ButtonReleased(button, _) => {
                        let pressed = matches!(event.event, EventType::ButtonPressed(..));

                        match button {
                            // Use dpad to focus button
                            Button::DPadLeft => {
                                if pressed {
                                    return self.move_focus(FocusDirection::Left);
                                }
                            }
                            Button::DPadRight => {
                                if pressed {
                                    return self.move_focus(FocusDirection::Right);
                                }
                            }
                            Button::DPadUp => {
                                if pressed {
                                    return self.move_focus(FocusDirection::Up);
                                }
                            }
                            Button::DPadDown => {
                                if pressed {
                                    return self.move_focus(FocusDirection::Down);
                                }
                            }
                            // Press current focused button on south button
                            Button::South => {
                                if let Some((_, _, key)) = self.find_focus() {
                                    if let Some(keycode) = key.keycode {
                                        return self.update(Message::Key {
                                            kind: key.kind,
                                            keycode,
                                            pressed,
                                        });
                                    }
                                }
                            }
                            // Close on east button
                            Button::East => {
                                return self.update(Message::Quit);
                            }
                            // Search for layout specific mappings
                            _ => {
                                if let Some(layout_layer) = self.layout_layer() {
                                    for row in layout_layer.rows.iter() {
                                        for key in row.iter() {
                                            if key.gamepad_mapping == Some(button) {
                                                if let Some(keycode) = key.keycode {
                                                    return self.update(Message::Key {
                                                        kind: key.kind,
                                                        keycode,
                                                        pressed,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
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

    fn view_window(&self, id: window::Id) -> Element<Message> {
        let cosmic_theme::Spacing {
            space_s,
            space_xs,
            space_xxs,
            ..
        } = theme::spacing();

        let element: Element<_> = if let Some(layout_layer) = self.layout_layer() {
            let mut grid = widget::column::with_capacity(layout_layer.rows.len() + 1);
            grid = grid.push(widget::row::with_children(vec![
                widget::space().width(Length::Fill).into(),
                if self.docked {
                    widget::button::icon(
                        widget::icon::from_svg_bytes(include_bytes!("../res/keyboard-undock.svg"))
                            .symbolic(true),
                    )
                    .on_press(Message::Dock(false))
                    .into()
                } else {
                    widget::button::icon(
                        widget::icon::from_svg_bytes(include_bytes!("../res/keyboard-dock.svg"))
                            .symbolic(true),
                    )
                    .on_press(Message::Dock(true))
                    .into()
                },
                widget::button::icon(widget::icon::from_name("window-minimize-symbolic"))
                    .on_press(Message::Hide)
                    .into(),
                widget::button::icon(widget::icon::from_name("window-close-symbolic"))
                    .on_press(Message::Quit)
                    .into(),
            ]));
            for layout_row in layout_layer.rows.iter() {
                let mut r = widget::row::with_capacity(layout_row.len() + 2);
                r = r.push(widget::space().width(Length::Fill));
                for key in layout_row.iter() {
                    let mut pressed = false;
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
                        if self.pressed.contains(&kc) {
                            pressed = true;
                        }
                    }

                    //TODO: adjust to match design
                    let style = {
                        use widget::button::Catalog;

                        fn adjust(
                            theme: &cosmic::Theme,
                            selected: bool,
                            mut style: widget::button::Style,
                        ) -> widget::button::Style {
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
                        }

                        theme::Button::Custom {
                            active: Box::new(move |focused, theme| {
                                adjust(
                                    theme,
                                    selected,
                                    if pressed {
                                        theme.pressed(focused, selected, &theme::Button::MenuItem)
                                    } else {
                                        theme.active(focused, selected, &theme::Button::MenuItem)
                                    },
                                )
                            }),
                            disabled: Box::new(move |theme| {
                                adjust(theme, selected, theme.disabled(&theme::Button::MenuItem))
                            }),
                            hovered: Box::new(move |focused, theme| {
                                adjust(
                                    theme,
                                    selected,
                                    if pressed {
                                        theme.pressed(focused, selected, &theme::Button::MenuItem)
                                    } else {
                                        theme.hovered(focused, selected, &theme::Button::MenuItem)
                                    },
                                )
                            }),
                            pressed: Box::new(move |focused, theme| {
                                adjust(
                                    theme,
                                    selected,
                                    theme.pressed(focused, selected, &theme::Button::MenuItem),
                                )
                            }),
                        }
                    };

                    let mut button_row = widget::row::with_capacity(3)
                        .align_y(Alignment::Center)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .push(widget::space().width(Length::Fill));

                    if self.gamepad_shown
                        && let Some(button) = &key.gamepad_mapping
                    {
                        let svg_opt = match button {
                            gilrs::Button::North => {
                                Some(include_str!("../res/gamepad-north-symbolic.svg"))
                            }
                            gilrs::Button::West => {
                                Some(include_str!("../res/gamepad-west-symbolic.svg"))
                            }
                            gilrs::Button::LeftTrigger2 => {
                                Some(include_str!("../res/gamepad-left-trigger-symbolic.svg"))
                            }
                            gilrs::Button::RightTrigger2 => {
                                Some(include_str!("../res/gamepad-right-trigger-symbolic.svg"))
                            }
                            gilrs::Button::LeftThumb => {
                                Some(include_str!("../res/gamepad-left-stick-symbolic.svg"))
                            }
                            _ => None,
                        };
                        if let Some(svg) = svg_opt {
                            button_row = button_row
                                .push(
                                    widget::icon(
                                        widget::icon::from_svg_bytes(svg.as_bytes()).symbolic(true),
                                    )
                                    .size(16),
                                )
                                .push(widget::space().width(space_xxs));
                        }
                    }

                    button_row = button_row
                        .push(if selected {
                            widget::text::heading(&key.name)
                        } else {
                            widget::text::body(&key.name)
                        })
                        .push(widget::space().width(Length::Fill));

                    let mut button = widget::button::custom(button_row)
                        .class(style)
                        .id(key.id.clone())
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
        let container = widget::container(element)
            .center(Length::Fill)
            .class(theme::Container::Background)
            .padding([space_xxs, space_s, space_xs, space_s]);
        if self.docked {
            container.into()
        } else {
            let surface_rect = if self.drag.dragging {
                self.drag.surface_rect
            } else {
                self.surface_rect
            };
            widget::container(
                container
                    .width(surface_rect.width)
                    .height(surface_rect.height),
            )
            .class(theme::Container::Transparent)
            .padding([surface_rect.y, 0.0, 0.0, surface_rect.x])
            .into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        struct WaylandSubscription;
        struct GilrsSubscription;

        Subscription::batch([
            event::listen_with(|event, status, _surface_id| match (event, status) {
                (
                    event::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    event::Status::Ignored,
                ) => Some(Message::DragStart),
                (
                    event::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                    event::Status::Ignored,
                ) => Some(Message::DragEnd),
                (
                    event::Event::Mouse(mouse::Event::CursorMoved { position }),
                    event::Status::Ignored,
                ) => Some(Message::DragMove(position)),
                (event::Event::Window(window::Event::Resized(size)), _) => {
                    Some(Message::Size(size))
                }
                _ => None,
            }),
            Subscription::run_with(TypeId::of::<WaylandSubscription>(), |_| {
                stream::channel(
                    128,
                    |output: futures::channel::mpsc::Sender<Message>| async move {
                        tokio::task::spawn_blocking(move || wayland::wayland_task(output))
                            .await
                            .unwrap();
                    },
                )
            }),
            ei::subscription().map(Message::Ei),
            Subscription::run_with(TypeId::of::<GilrsSubscription>(), |_| {
                stream::channel(
                    128,
                    |mut output: futures::channel::mpsc::Sender<Message>| async move {
                        tokio::task::spawn_blocking(move || {
                            let mut gilrs = gilrs::Gilrs::new().unwrap();
                            loop {
                                // Examine new events
                                while let Some(event) = gilrs.next_event_blocking(None) {
                                    futures::executor::block_on(async {
                                        output.send(Message::Gilrs(event)).await
                                    })
                                    .unwrap();
                                }
                            }
                        })
                        .await
                        .unwrap();
                    },
                )
            }),
        ])
    }
}

// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    Application, Element,
    app::{Core, Settings, Task},
    cosmic_config::{self, CosmicConfigEntry},
    cosmic_theme, executor,
    iced::{
        Alignment, Length, Limits, Point, Rectangle, Size, Subscription, Vector, event,
        futures::{self, SinkExt},
        mouse,
        platform_specific::{
            runtime::wayland::layer_surface::{IcedMargin, IcedOutput, SctkLayerSurfaceSettings},
            shell::{
                commands::blur::blur,
                wayland::commands::layer_surface::{
                    Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
                    set_input_zone,
                },
            },
        },
        runtime::platform_specific::wayland::CornerRadius,
        stream,
        touch::{self, Finger},
        window,
    },
    surface::corner_radius::rounded_rect_strips,
    theme, widget,
};
use reis::ei::keyboard::KeyState;
use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    process,
    time::Instant,
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

#[derive(Default)]
pub struct GamepadMouse {
    dx: Option<f32>,
    dy: Option<f32>,
    scrolling: bool,
    update: Option<Instant>,
}

impl GamepadMouse {
    pub fn frame(&mut self, instant: Instant) -> Option<(f32, f32)> {
        if self.dx.is_none() && self.dy.is_none() {
            self.update = None;
            return None;
        }

        let duration = instant
            .checked_duration_since(self.update.unwrap_or(instant))
            .unwrap_or_default()
            .as_secs_f32();
        self.update = Some(instant);
        Some((
            self.dx.unwrap_or_default() * duration,
            -self.dy.unwrap_or_default() * duration,
        ))
    }
}

#[derive(Default)]
pub struct GamepadState {
    pub axes: HashMap<gilrs::Axis, GamepadAxisDirection>,
    pub buttons: HashSet<gilrs::Button>,
    pub mouse: GamepadMouse,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Message {
    Dock(bool),
    DragStart(Option<Finger>),
    DragMove(Option<Finger>, Point),
    DragEnd(Option<Finger>),
    Frame(Instant),
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
    finger: Option<Finger>,
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
    pressed: HashMap<layout::KeyCode, layout::KeyKind>,
    sticky: HashSet<layout::KeyCode>,
    size: Size,
    surface_auto_pos: bool,
    surface_id: Option<window::Id>,
    surface_rect: Rectangle,
    xkb_state: Option<xkb::State>,
    // TODO reis state
    ei_conn: Option<reis::event::Connection>,
    ei_button: Option<(reis::ei::Device, reis::ei::Button)>,
    ei_keyboard: Option<(reis::ei::Device, reis::ei::Keyboard)>,
    ei_pointer: Option<(reis::ei::Device, reis::ei::Pointer)>,
    ei_scroll: Option<(reis::ei::Device, reis::ei::Scroll)>,
    gamepads: HashMap<gilrs::GamepadId, GamepadState>,
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
        // Without layouts the surface would be created with height 0
        if self.surface_id.is_some() || self.layouts.is_none() {
            return Task::none();
        }

        self.surface_rect.width = 0.0;
        self.surface_rect.height = 0.0;
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
            settings.input_zone = Some(vec![self.surface_rect]);
            settings.size = None;
            settings.exclusive_zone = 0;
        }

        log::info!("get_layer_surface");
        get_layer_surface(settings)
    }

    pub fn key_level<'a>(&'a self, key: &'a layout::Key) -> &'a layout::KeyLevel {
        let mut level = 0;
        if let Some(keycode) = key.keycode
            && let Some(xkb_state) = &self.xkb_state
        {
            level = usize::try_from(xkb_state.key_get_level(keycode.xkb(), self.group))
                .unwrap_or_default();
            if level >= key.levels.len() {
                log::debug!("key {:?} does not have level {}", key.levels[0].name, level);
                level = 0;
            }
        }
        &key.levels[level]
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

    pub fn release_all(&mut self) {
        let pressed = self.pressed.clone();
        for (keycode, kind) in pressed {
            //TODO: ensure this task is none
            let _ = self.update(Message::Key {
                kind,
                keycode,
                pressed: false,
            });
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
            pressed: HashMap::new(),
            sticky: HashSet::new(),
            size: Size::default(),
            surface_auto_pos: true,
            surface_id: None,
            surface_rect: Rectangle::default(),
            xkb_state: None,
            ei_conn: None,
            ei_button: None,
            ei_keyboard: None,
            ei_pointer: None,
            ei_scroll: None,
            gamepads: HashMap::new(),
            gamepad_shown: false,
        };

        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Dock(dock) => {
                self.release_all();
                if dock != self.docked {
                    let hide_task = self.hide();
                    self.docked = dock;
                    let show_task = self.show();
                    return Task::batch([hide_task, show_task]);
                }
            }
            Message::DragStart(finger) => {
                self.release_all();
                if !self.docked && !self.drag.dragging {
                    self.drag = DragState::default();
                    self.drag.dragging = true;
                    self.drag.finger = finger;
                    self.drag.surface_rect = self.surface_rect;
                }
            }
            Message::DragMove(finger, point) => {
                self.release_all();
                if !self.docked && self.drag.dragging && self.drag.finger == finger {
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
                        // Do not auto position on display after drag
                        self.surface_auto_pos = false;
                    } else {
                        self.drag.start_pos = self.drag.mouse_pos;
                    }
                    if let Some(surface_id) = self.surface_id {
                        let t = cosmic::theme::active();
                        let theme = t.cosmic();
                        let rad_s = theme.radius_s();
                        return blur(
                            surface_id,
                            Some(rounded_rect_strips(
                                self.drag.surface_rect,
                                CornerRadius {
                                    top_left: rad_s[0] as u32,
                                    top_right: rad_s[1] as u32,
                                    bottom_right: rad_s[2] as u32,
                                    bottom_left: rad_s[3] as u32,
                                },
                            )),
                        )
                        .discard();
                    }
                }
            }
            Message::DragEnd(finger) => {
                self.release_all();
                if !self.docked && self.drag.dragging && self.drag.finger == finger {
                    self.surface_rect = self.drag.surface_rect;
                    self.drag = DragState::default();
                    if let Some(surface_id) = self.surface_id {
                        return set_input_zone(surface_id, Some(vec![self.surface_rect]));
                    }
                }
            }
            Message::Frame(instant) => {
                let mut frame_pointer = false;
                let mut frame_scroll = false;
                for state in self.gamepads.values_mut() {
                    if let Some((dx, dy)) = state.mouse.frame(instant) {
                        if state.mouse.scrolling {
                            if let Some((_, scroll)) = &self.ei_scroll {
                                //TODO: find ideal speed
                                let speed = 1000.0;
                                scroll.scroll(dx * speed, dy * speed);
                                frame_scroll = true;
                            }
                        } else {
                            if let Some((_, pointer)) = &self.ei_pointer {
                                //TODO: find ideal speed
                                let speed = 2000.0;
                                pointer.motion_relative(dx * speed, dy * speed);
                                frame_pointer = true;
                            }
                        }
                    }
                }

                if frame_pointer && let Some((device, _)) = &self.ei_pointer {
                    // TODO device frame
                    device.frame(0, 1); // TODO
                }
                if frame_scroll && let Some((device, _)) = &self.ei_scroll {
                    // TODO device frame
                    device.frame(0, 1); // TODO
                }
                if frame_pointer | frame_scroll {
                    self.ei_conn
                        .as_ref()
                        .unwrap()
                        .flush()
                        .expect("failed to flush EI connection");
                }
            }
            Message::Focus(id) => {
                self.focus = Some(id.clone());
                return widget::button::focus(id);
            }
            Message::Hide => {
                self.release_all();
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
                            self.pressed.insert(keycode, kind);
                        } else {
                            self.pressed.remove(&keycode);
                        }
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
            }
            Message::Quit => {
                process::exit(0);
            }
            Message::SeatImActive { seat_id, active } => {
                log::info!("{} active: {}", seat_id, active);
                if active {
                    if !self.ignore_activate {
                        return self.show();
                    }
                } else {
                    self.ignore_activate = false;
                }
            }
            Message::Size(size) => {
                log::info!("size: {:?}", size);
                if let Some(surface_id) = self.surface_id
                    && !self.docked
                {
                    let mut tasks = Vec::with_capacity(2);
                    self.size = size;
                    if self.surface_auto_pos {
                        // Automatically position at center bottom when first floated
                        self.surface_rect.x = (size.width - self.surface_rect.width) / 2.0;
                        self.surface_rect.y = (size.height - self.surface_rect.height);
                        tasks.push(set_input_zone(surface_id, Some(vec![self.surface_rect])));
                    }
                    let t = cosmic::theme::active();
                    let theme = t.cosmic();
                    let rad_s = theme.radius_s();
                    tasks.push(
                        blur(
                            surface_id,
                            Some(rounded_rect_strips(
                                self.surface_rect,
                                CornerRadius {
                                    top_left: rad_s[0] as u32,
                                    top_right: rad_s[1] as u32,
                                    bottom_right: rad_s[2] as u32,
                                    bottom_left: rad_s[3] as u32,
                                },
                            )),
                        )
                        .discard(),
                    );
                    return Task::batch(tasks);
                }
            }
            Message::Ei(evt) => {
                match evt {
                    ei::Msg::Connection(conn) => {
                        self.ei_conn = Some(conn);
                    }
                    ei::Msg::Event(reis::event::EiEvent::SeatAdded(evt)) => {
                        use reis::event::DeviceCapability;

                        log::info!("{:?}", evt);

                        evt.seat.bind_capabilities(
                            (DeviceCapability::Keyboard
                                | DeviceCapability::Button
                                | DeviceCapability::Pointer
                                | DeviceCapability::Scroll)
                                .into(),
                        );
                        let _ = self.ei_conn.as_ref().unwrap().flush();
                    }
                    ei::Msg::Event(reis::event::EiEvent::DeviceAdded(evt)) => {
                        use reis::event::DeviceCapability;

                        log::info!("{:?}", evt);
                        let mut start = false;

                        if evt.device.has_capability(DeviceCapability::Button) {
                            log::info!("  has button");
                            self.ei_button = Some((
                                evt.device.device().clone(),
                                evt.device.interface::<reis::ei::Button>().unwrap(),
                            ));
                            start = true;
                        }

                        if evt.device.has_capability(DeviceCapability::Pointer) {
                            log::info!("  has pointer");
                            self.ei_pointer = Some((
                                evt.device.device().clone(),
                                evt.device.interface::<reis::ei::Pointer>().unwrap(),
                            ));
                            start = true;
                        }

                        if evt.device.has_capability(DeviceCapability::Scroll) {
                            log::info!("  has scroll");
                            self.ei_scroll = Some((
                                evt.device.device().clone(),
                                evt.device.interface::<reis::ei::Scroll>().unwrap(),
                            ));
                            start = true;
                        }

                        if evt.device.has_capability(DeviceCapability::Keyboard) {
                            log::info!("  has keyboard");
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

                        // This starts emulating if a non-keyboard device is found. The keyboard type does it above
                        if start {
                            let serial = self.ei_conn.as_ref().unwrap().serial();
                            evt.device.device().start_emulating(0, serial);
                        }
                    }
                    ei::Msg::Event(reis::event::EiEvent::KeyboardModifiers(evt)) => {
                        self.group = evt.group;
                        if let Some(xkb_state) = &mut self.xkb_state {
                            xkb_state.update_mask(
                                evt.depressed,
                                evt.latched,
                                evt.locked,
                                evt.group,
                                evt.group,
                                evt.group,
                            );
                        }
                    }
                    _ => {}
                }
            }
            Message::Gilrs(event) => {
                let state = self
                    .gamepads
                    .entry(event.id)
                    .or_insert_with(|| GamepadState::default());

                match event.event {
                    EventType::ButtonPressed(button, _) => {
                        state.buttons.insert(button);
                    }
                    EventType::ButtonReleased(button, _) => {
                        state.buttons.remove(&button);
                    }
                    _ => {}
                }

                // Only handle gamepad events if surface is visible
                if self.surface_id.is_none() {
                    // Show on Start+Select gesture
                    if state.buttons.contains(&Button::Start)
                        && state.buttons.contains(&Button::Select)
                        && self.surface_id.is_none()
                    {
                        return self.show();
                    }

                    // Clear axes and mouse state
                    for state in self.gamepads.values_mut() {
                        state.axes.clear();
                        state.mouse = Default::default();
                    }

                    return Task::none();
                }

                use gilrs::{Axis, Button, EventType};

                // Show the gamepad mappings after any gamepad event
                self.gamepad_shown = true;

                match event.event {
                    EventType::AxisChanged(axis, value, _) => {
                        // Emulate a dpad press on left axis movement
                        const AXIS_OFF: f32 = 0.25;
                        const AXIS_ON: f32 = 0.5;
                        let last_dir = state.axes.get(&axis).copied().unwrap_or_default();
                        let dir = if value < -AXIS_ON {
                            GamepadAxisDirection::Negative
                        } else if value > AXIS_ON {
                            GamepadAxisDirection::Positive
                        } else if value > -AXIS_OFF && value < AXIS_OFF {
                            GamepadAxisDirection::Center
                        } else {
                            last_dir
                        };
                        state.axes.insert(axis, dir);

                        // Emulate a mouse on right axis movement
                        const MOUSE_DEADZONE: f32 = 0.15;

                        match axis {
                            Axis::LeftStickX if last_dir != dir => match dir {
                                GamepadAxisDirection::Negative => {
                                    return self.move_focus(FocusDirection::Left);
                                }
                                GamepadAxisDirection::Positive => {
                                    return self.move_focus(FocusDirection::Right);
                                }
                                _ => {}
                            },
                            Axis::LeftStickY if last_dir != dir => match dir {
                                GamepadAxisDirection::Negative => {
                                    return self.move_focus(FocusDirection::Down);
                                }
                                GamepadAxisDirection::Positive => {
                                    return self.move_focus(FocusDirection::Up);
                                }
                                _ => {}
                            },
                            Axis::RightStickX => {
                                if value < -MOUSE_DEADZONE {
                                    state.mouse.dx = Some(value + MOUSE_DEADZONE);
                                } else if value > MOUSE_DEADZONE {
                                    state.mouse.dx = Some(value - MOUSE_DEADZONE);
                                } else {
                                    state.mouse.dx = None;
                                }
                            }
                            Axis::RightStickY => {
                                if value < -MOUSE_DEADZONE {
                                    state.mouse.dy = Some(value + MOUSE_DEADZONE);
                                } else if value > MOUSE_DEADZONE {
                                    state.mouse.dy = Some(value - MOUSE_DEADZONE);
                                } else {
                                    state.mouse.dy = None;
                                }
                            }
                            _ => {}
                        }

                        if state.mouse.dx.is_none() && state.mouse.dy.is_none() {
                            state.mouse.update = None;
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
                                        let key_level = self.key_level(&key);
                                        return self.update(Message::Key {
                                            kind: key_level.kind,
                                            keycode,
                                            pressed,
                                        });
                                    }
                                }
                            }
                            // Hide on east button
                            Button::East => {
                                return self.update(Message::Hide);
                            }
                            // Left click on R1, right click on L1 (intentional)
                            Button::LeftTrigger | Button::RightTrigger => {
                                let index = match button {
                                    Button::RightTrigger => {
                                        // BTN_LEFT
                                        0x110
                                    }
                                    Button::LeftTrigger => {
                                        // BTN_RIGHT
                                        0x111
                                    }
                                    _ => {
                                        return Task::none();
                                    }
                                };
                                if let Some((device, button)) = &self.ei_button {
                                    button.button(
                                        index,
                                        if pressed {
                                            reis::ei::button::ButtonState::Press
                                        } else {
                                            reis::ei::button::ButtonState::Released
                                        },
                                    );

                                    // TODO device frame
                                    device.frame(0, 1); // TODO
                                    self.ei_conn
                                        .as_ref()
                                        .unwrap()
                                        .flush()
                                        .expect("failed to flush EI connection");
                                }
                            }
                            // Toggle scrolling on right thumb
                            Button::RightThumb => {
                                if pressed {
                                    state.mouse.scrolling = !state.mouse.scrolling;
                                }
                            }
                            // Toggle docking on select
                            Button::Select => {
                                if pressed {
                                    return self.update(Message::Dock(!self.docked));
                                }
                            }
                            // Search for layout specific mappings
                            _ => {
                                if let Some(layout_layer) = self.layout_layer() {
                                    for row in layout_layer.rows.iter() {
                                        for key in row.iter() {
                                            if key.gamepad_mapping == Some(button) {
                                                if let Some(keycode) = key.keycode {
                                                    return self.update(Message::Key {
                                                        // Use normal type to avoid sticky modifiers
                                                        kind: layout::KeyKind::Normal,
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
                widget::button::icon(
                    widget::icon::from_svg_bytes(include_bytes!(
                        "../res/preferences-desktop-keyboard-symbolic.svg"
                    ))
                    .symbolic(true),
                )
                .into(),
                widget::button::icon(widget::icon::from_name("view-more-symbolic")).into(),
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
                    let key_level = self.key_level(&key);

                    let mut pressed = false;
                    let mut selected = false;
                    if let Some(kc) = key.keycode {
                        if let layout::KeyKind::Mod { name, sticky } = key_level.kind {
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
                        if self.pressed.contains_key(&kc) {
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
                                    .size(24),
                                )
                                .push(widget::space().width(space_xxs));
                        }
                    }

                    if let Some(icon) = &key_level.icon {
                        button_row = button_row.push(widget::icon(icon.clone()).size(20));
                    } else {
                        button_row = button_row.push(
                            widget::Text::new(&key_level.name)
                                .size(if key_level.name.chars().count() <= 1 {
                                    18
                                } else {
                                    16
                                })
                                .font(if selected {
                                    cosmic::font::semibold()
                                } else {
                                    cosmic::font::default()
                                }),
                        );
                    }
                    button_row = button_row.push(widget::space().width(Length::Fill));

                    let mut button = widget::button::custom(button_row)
                        .class(style)
                        .id(key.id.clone())
                        .selected(selected)
                        .width(Length::Fill)
                        .height(Length::Fill);

                    if let Some(keycode) = key.keycode {
                        button = button
                            .on_press_down(Message::Key {
                                kind: key_level.kind,
                                keycode,
                                pressed: true,
                            })
                            .on_press(Message::Key {
                                kind: key_level.kind,
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
                //TODO: use mouse position at start of drag
                (
                    event::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    event::Status::Ignored,
                ) => Some(Message::DragStart(None)),
                (
                    event::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                    event::Status::Ignored,
                ) => Some(Message::DragEnd(None)),
                (
                    event::Event::Mouse(mouse::Event::CursorMoved { position }),
                    event::Status::Ignored,
                ) => Some(Message::DragMove(None, position)),
                //TODO: use touch position at start of drag
                (
                    event::Event::Touch(touch::Event::FingerPressed { id, .. }),
                    event::Status::Ignored,
                ) => Some(Message::DragStart(Some(id))),
                (
                    event::Event::Touch(touch::Event::FingerMoved { id, position }),
                    event::Status::Ignored,
                ) => Some(Message::DragMove(Some(id), position)),
                (
                    event::Event::Touch(
                        touch::Event::FingerLifted { id, .. } | touch::Event::FingerLost { id, .. },
                    ),
                    event::Status::Ignored,
                ) => Some(Message::DragEnd(Some(id))),
                (
                    event::Event::Window(
                        window::Event::Opened { size, .. } | window::Event::Resized(size),
                    ),
                    _,
                ) => Some(Message::Size(size)),
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
            if self
                .gamepads
                .values()
                .any(|state| state.mouse.dx.is_some() || state.mouse.dy.is_some())
            {
                window::frames().map(|(_surface_id, instant)| Message::Frame(instant))
            } else {
                Subscription::none()
            },
        ])
    }
}

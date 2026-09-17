// SPDX-License-Identifier: GPL-3.0-only

use cosmic::widget;
use cosmic_osk_config::Config;
use xkbcommon::xkb::{self, Keysym};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct KeyCode(pub xkb::Keycode);

impl KeyCode {
    pub fn xkb(&self) -> xkb::Keycode {
        self.0
    }

    pub fn evdev(&self) -> u32 {
        u32::from(self.0)
            .checked_sub(8)
            .expect("XKB keycode should be greater than 8")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum KeyKind {
    Mod {
        // Name of modifier
        name: &'static str,
        // Sticky or not
        sticky: bool,
    },
    Normal,
}

#[derive(Clone, Debug)]
pub struct KeyLevel {
    pub name: String,
    pub kind: KeyKind,
    pub icon: Option<widget::icon::Handle>,
}

impl KeyLevel {
    pub fn for_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: KeyKind::Normal,
            icon: None,
        }
    }

    pub fn for_sym(sym: Keysym) -> Self {
        // Default to keysym name
        let mut name = xkb::keysym_get_name(sym);

        // Prefer keysym char
        if let Some(char) = sym.key_char() {
            if !char.is_control() {
                name = char.to_string();
            }
        }

        // Translate some keysym names
        name = match name.trim_start_matches("KP_") {
            "Alt_L" | "Alt_R" | "Meta_L" => "Alt",
            "Caps_Lock" => "Caps",
            "Control_L" | "Control_R" => "Ctrl",
            "Delete" => "Del",
            "Escape" => "Esc",
            "ISO_Level3_Shift" => "AltGr",
            "Insert" => "Ins",
            "Multi_key" => "Comp",
            "Next" => "PgDn",
            "Num_Lock" => "Num",
            "Prior" => "PgUp",
            "Super_L" | "Super_R" => "Super",
            "Tab" | "ISO_Left_Tab" => "Tab",
            " " => "Space",
            // Dead keys sorted by symbol number
            "dead_grave" => "\u{25CC}\u{0300}",
            "dead_acute" => "\u{25CC}\u{0301}",
            "dead_circumflex" => "\u{25CC}\u{0302}",
            "dead_tilde" => "\u{25CC}\u{0303}",
            "dead_macron" => "\u{25CC}\u{0304}",
            "dead_breve" => "\u{25CC}\u{0306}",
            "dead_abovedot" => "\u{25CC}\u{0307}",
            "dead_diaeresis" => "\u{25CC}\u{0308}",
            "dead_abovering" => "\u{25CC}\u{030A}",
            "dead_doubleacute" => "\u{25CC}\u{030B}",
            "dead_caron" => "\u{25CC}\u{030C}",
            "dead_cedilla" => "\u{25CC}\u{0327}",
            "dead_ogonek" => "\u{25CC}\u{0328}",
            //"dead_iota" => "\u{25CC}\u{03XX}",
            //"dead_voiced_sound" => "\u{25CC}\u{03XX}",
            //"dead_semivoiced_sound" => "\u{25CC}\u{03XX}",
            "dead_belowdot" => "\u{25CC}\u{0323}",
            "dead_hook" => "\u{25CC}\u{0309}",
            "dead_horn" => "\u{25CC}\u{031B}",
            "dead_stroke" => "\u{25CC}\u{0335}",
            "dead_abovecomma" => "\u{25CC}\u{0313}",
            "dead_abovereversedcomma" => "\u{25CC}\u{0314}",
            "dead_doublegrave" => "\u{25CC}\u{030F}",
            "dead_belowring" => "\u{25CC}\u{0325}",
            "dead_belowmacron" => "\u{25CC}\u{0331}",
            "dead_belowcircumflex" => "\u{25CC}\u{032D}",
            "dead_belowtilde" => "\u{25CC}\u{0330}",
            "dead_belowbreve" => "\u{25CC}\u{032E}",
            "dead_belowdiaeresis" => "\u{25CC}\u{0324}",
            "dead_invertedbreve" => "\u{25CC}\u{0311}",
            "dead_belowcomma" => "\u{25CC}\u{0326}",
            //"dead_currency" => "\u{25CC}\u{03XX}",
            "dead_lowline" => "\u{25CC}\u{0332}",
            "dead_aboveverticalline" => "\u{25CC}\u{030D}",
            "dead_belowverticalline" => "\u{25CC}\u{0329}",
            "dead_longsolidusoverlay" => "\u{25CC}\u{0338}",
            "dead_a" => "\u{25CC}\u{0363}",
            //"dead_A" => "\u{25CC}\u{03XX}",
            "dead_e" => "\u{25CC}\u{0364}",
            //"dead_E" => "\u{25CC}\u{03XX}",
            "dead_i" => "\u{25CC}\u{0365}",
            //"dead_I" => "\u{25CC}\u{03XX}",
            "dead_o" => "\u{25CC}\u{0366}",
            //"dead_O" => "\u{25CC}\u{03XX}",
            "dead_u" => "\u{25CC}\u{0367}",
            //"dead_U" => "\u{25CC}\u{03XX}",
            //"dead_schwa" => "\u{25CC}\u{03XX}",
            //"dead_SCHWA" => "\u{25CC}\u{03XX}",
            //"dead_greek" => "\u{25CC}\u{03XX}",
            //"dead_hamza" => "\u{25CC}\u{03XX}",
            other => other,
        }
        .to_string();

        if name.starts_with("dead_") {
            log::warn!("unknown dead key {}", name);
        }

        //TODO: get modifier names from xkbcommon
        let kind = match sym {
            // Press these modifiers until a normal key is pressed
            Keysym::Alt_L | Keysym::Alt_R => KeyKind::Mod {
                name: xkb::MOD_NAME_ALT,
                sticky: true,
            },
            Keysym::Control_L | Keysym::Control_R => KeyKind::Mod {
                name: xkb::MOD_NAME_CTRL,
                sticky: true,
            },
            Keysym::ISO_Level3_Shift => KeyKind::Mod {
                name: xkb::MOD_NAME_ISO_LEVEL3_SHIFT,
                sticky: true,
            },
            Keysym::Shift_L | Keysym::Shift_R => KeyKind::Mod {
                name: xkb::MOD_NAME_SHIFT,
                sticky: true,
            },
            Keysym::Super_L | Keysym::Super_R => KeyKind::Mod {
                name: xkb::MOD_NAME_LOGO,
                sticky: true,
            },
            // Caps-lock already toggles itself
            Keysym::Caps_Lock => KeyKind::Mod {
                name: xkb::MOD_NAME_CAPS,
                sticky: false,
            },
            // Num-lock already toggles itself
            Keysym::Num_Lock => KeyKind::Mod {
                name: xkb::MOD_NAME_NUM,
                sticky: false,
            },
            // Normal keys
            _ => KeyKind::Normal,
        };

        let icon = match sym {
            Keysym::BackSpace => Some(widget::icon::from_name("edit-clear-symbolic").handle()),
            Keysym::Return => Some(
                widget::icon::from_svg_bytes(include_bytes!("../res/keycap-return.svg"))
                    .symbolic(true),
            ),
            Keysym::Down => Some(widget::icon::from_name("pan-down-symbolic").handle()),
            Keysym::Left => Some(widget::icon::from_name("pan-start-symbolic").handle()),
            Keysym::Shift_L | Keysym::Shift_R => Some(
                widget::icon::from_svg_bytes(include_bytes!("../res/keycap-shift.svg"))
                    .symbolic(true),
            ),
            Keysym::Right => Some(widget::icon::from_name("pan-end-symbolic").handle()),
            Keysym::Up => Some(widget::icon::from_name("pan-up-symbolic").handle()),
            Keysym::XF86_AudioNext => {
                Some(widget::icon::from_name("media-seek-forward-symbolic").handle())
            }
            Keysym::XF86_AudioPlay => {
                Some(widget::icon::from_name("media-playback-start-symbolic").handle())
            }
            Keysym::XF86_AudioPause => {
                Some(widget::icon::from_name("media-playback-pause-symbolic").handle())
            }
            Keysym::XF86_AudioPrev => {
                Some(widget::icon::from_name("media-seek-backward-symbolic").handle())
            }
            _ => None,
        };

        Self { name, kind, icon }
    }
}

#[derive(Clone, Debug)]
pub struct Key {
    pub id: widget::Id,
    pub levels: Vec<KeyLevel>,
    pub width: f32,
    pub keycode: Option<KeyCode>,
    pub gamepad_mapping: Option<gilrs::Button>,
    pub spacer: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub rows: Vec<Vec<Key>>,
}

impl Layout {
    pub fn all(keymap: &xkb::Keymap, config: &Config) -> Option<Vec<Self>> {
        if keymap.num_layouts() == 0 {
            None
        } else {
            Some(
                (0..keymap.num_layouts())
                    .map(|layout| Self::new(keymap, config, layout))
                    .collect(),
            )
        }
    }

    fn key_rows(config: &Config) -> Vec<Vec<(&'static str, f32)>> {
        macro_rules! k {
            ($name:expr) => {
                ($name, 1.0)
            };
            ($name:expr, $width:expr) => {
                ($name, $width)
            };
        }

        let mut key_rows = Vec::new();
        if config.function_row {
            key_rows.push(vec![
                k!("ESC"),
                k!("FK01"),
                k!("FK02"),
                k!("FK03"),
                k!("FK04"),
                k!("FK05"),
                k!("FK06"),
                k!("FK07"),
                k!("FK08"),
                k!("FK09"),
                k!("FK10"),
                k!("FK11"),
                k!("FK12"),
                k!("DELE", 2.0),
            ]);
        }
        key_rows.push(vec![
            k!("TLDE"),
            k!("AE01"),
            k!("AE02"),
            k!("AE03"),
            k!("AE04"),
            k!("AE05"),
            k!("AE06"),
            k!("AE07"),
            k!("AE08"),
            k!("AE09"),
            k!("AE10"),
            k!("AE11"),
            k!("AE12"),
            k!("BKSP", 2.0),
        ]);
        key_rows.push(vec![
            k!("TAB", 1.5),
            k!("AD01"),
            k!("AD02"),
            k!("AD03"),
            k!("AD04"),
            k!("AD05"),
            k!("AD06"),
            k!("AD07"),
            k!("AD08"),
            k!("AD09"),
            k!("AD10"),
            k!("AD11"),
            k!("AD12"),
            k!("BKSL", 1.5),
        ]);
        key_rows.push(vec![
            k!("CAPS", 1.75),
            k!("AC01"),
            k!("AC02"),
            k!("AC03"),
            k!("AC04"),
            k!("AC05"),
            k!("AC06"),
            k!("AC07"),
            k!("AC08"),
            k!("AC09"),
            k!("AC10"),
            k!("AC11"),
            k!("RTRN", 2.25),
        ]);
        key_rows.push(vec![
            k!("LFSH", 2.25),
            k!("AB01"),
            k!("AB02"),
            k!("AB03"),
            k!("AB04"),
            k!("AB05"),
            k!("AB06"),
            k!("AB07"),
            k!("AB08"),
            k!("AB09"),
            k!("AB10"),
            k!("RTSH", 1.75),
            k!("UP"),
        ]);
        key_rows.push(vec![
            k!("LCTL", 1.25),
            k!("LWIN", 1.25),
            k!("LALT", 1.25),
            k!("SPCE", 5.5),
            k!("RALT", 1.25),
            k!("MENU", 1.25),
            k!("RCTL", 1.25),
            k!("LEFT"),
            k!("DOWN"),
            k!("RGHT"),
        ]);
        if config.function_row {
            key_rows[0].push(k!("HOME"));
            key_rows[1].push(k!("PGUP"));
            key_rows[2].push(k!("PGDN"));
            key_rows[3].push(k!("END"));
            key_rows[4].push(k!("INS"));
        } else {
            key_rows[0].extend_from_slice(&[k!("HOME")]);
            key_rows[1].extend_from_slice(&[k!("PGUP")]);
            key_rows[2].extend_from_slice(&[k!("PGDN")]);
            key_rows[3].extend_from_slice(&[k!("END")]);
        }
        if config.numpad {
            //TODO: come up with a way to have multi-row keys for KPAD and KPEN?
            let mut row = 0;
            if config.function_row {
                // I171 = NEXTSONG
                // I172 = PLAYPAUSE
                // I173 = PREVIOUSSONG
                key_rows[row].extend_from_slice(&[k!("PRSC"), k!("I173"), k!("I172"), k!("I171")]);
                row += 1;
            }
            key_rows[row].extend_from_slice(&[k!("NMLK"), k!("KPDV"), k!("KPMU"), k!("KPSU")]);
            key_rows[row + 1].extend_from_slice(&[k!("KP7"), k!("KP8"), k!("KP9"), k!("KPAD")]);
            key_rows[row + 2].extend_from_slice(&[k!("KP4"), k!("KP5"), k!("KP6"), k!("KPEQ")]);
            key_rows[row + 3].extend_from_slice(&[k!("KP1"), k!("KP2"), k!("KP3"), k!("KPDL")]);
            key_rows[row + 4].extend_from_slice(&[k!("KP0", 2.0), k!("KPEN", 2.0)]);
        }
        key_rows
    }

    fn new(keymap: &xkb::Keymap, config: &Config, layout: u32) -> Self {
        assert!(keymap.num_layouts() > layout);

        let mut rows = Vec::new();
        for key_row in Self::key_rows(config) {
            let mut row = Vec::with_capacity(key_row.len());
            for &(keyname, width) in key_row.iter() {
                let gamepad_mapping = match keyname {
                    "BKSP" => Some(gilrs::Button::West),
                    "CAPS" => Some(gilrs::Button::LeftThumb),
                    "LFSH" => Some(gilrs::Button::LeftTrigger2),
                    "RTRN" => Some(gilrs::Button::RightTrigger2),
                    "SPCE" => Some(gilrs::Button::North),
                    _ => None,
                };

                let spacer = match keyname {
                    "PRSC" | "NMLK" | "KP7" | "KP4" | "KP1" | "KP0" => true,
                    _ => false,
                };

                let mut key = Key {
                    id: widget::Id::unique(),
                    levels: vec![KeyLevel::for_name(keyname)],
                    width,
                    keycode: None,
                    gamepad_mapping,
                    spacer,
                };

                match keymap.key_by_name(keyname) {
                    Some(kc) => {
                        key.keycode = Some(KeyCode(kc));

                        for level in 0..keymap.num_levels_for_key(kc, layout) as usize {
                            while key.levels.len() <= level {
                                key.levels.push(KeyLevel::for_name(keyname));
                            }

                            let syms = keymap.key_get_syms_by_level(kc, layout, level as u32);
                            if let Some(sym) = syms.get(0) {
                                key.levels[level] = KeyLevel::for_sym(*sym);
                            }
                        }
                    }
                    None => {
                        eprintln!("cannot find keycode for {:?} in keymap", keyname);
                    }
                }

                row.push(key);
            }
            rows.push(row);
        }
        Layout { rows }
    }
}

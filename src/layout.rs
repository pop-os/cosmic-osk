// SPDX-License-Identifier: GPL-3.0-only

use cosmic::widget;
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
            "dead_belowring" => "\u{25CC}\u{0326}",
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
}

#[derive(Clone, Copy, Debug)]
pub struct Setup {
    pub numpad: bool,
}

impl Setup {
    pub fn key_rows(&self) -> Vec<Vec<&'static str>> {
        let mut key_rows = Vec::new();
        key_rows.push(vec![
            "ESC", "FK01", "FK02", "FK03", "FK04", "FK05", "FK06", "FK07", "FK08", "FK09", "FK10",
            "FK11", "FK12", "DELE", "HOME",
        ]);
        key_rows.push(vec![
            "TLDE", "AE01", "AE02", "AE03", "AE04", "AE05", "AE06", "AE07", "AE08", "AE09", "AE10",
            "AE11", "AE12", "BKSP", "PGUP",
        ]);
        key_rows.push(vec![
            "TAB", "AD01", "AD02", "AD03", "AD04", "AD05", "AD06", "AD07", "AD08", "AD09", "AD10",
            "AD11", "AD12", "BKSL", "PGDN",
        ]);
        key_rows.push(vec![
            "CAPS", "AC01", "AC02", "AC03", "AC04", "AC05", "AC06", "AC07", "AC08", "AC09", "AC10",
            "AC11", "RTRN", "END",
        ]);
        key_rows.push(vec![
            "LFSH", "AB01", "AB02", "AB03", "AB04", "AB05", "AB06", "AB07", "AB08", "AB09", "AB10",
            "RTSH", "UP", "INS",
        ]);
        key_rows.push(vec![
            "LCTL", "LALT", "LWIN", "SPCE", "RALT", "RWIN", "RCTL", "LEFT", "DOWN", "RGHT",
        ]);
        if self.numpad {
            //TODO: come up with a way to have multi-row keys for KPAD and KPEN
            key_rows[0].extend_from_slice(&["PRSC", "I173", "I172", "I171"]);
            key_rows[1].extend_from_slice(&["NMLK", "KPDV", "KPMU", "KPSU"]);
            key_rows[2].extend_from_slice(&["KP7", "KP8", "KP9", "KPAD"]);
            key_rows[3].extend_from_slice(&["KP4", "KP5", "KP6", "KPAD"]);
            key_rows[4].extend_from_slice(&["KP1", "KP2", "KP3", "KPEN"]);
            key_rows[5].extend_from_slice(&["KP0", "KPDL", "KPEQ", "KPEN"]);
        }
        key_rows
    }
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub rows: Vec<Vec<Key>>,
}

impl Layout {
    pub fn all(keymap: &xkb::Keymap) -> Option<Vec<Self>> {
        if keymap.num_layouts() == 0 {
            None
        } else {
            Some(
                (0..keymap.num_layouts())
                    .map(|layout| Self::new(keymap, layout))
                    .collect(),
            )
        }
    }

    fn new(keymap: &xkb::Keymap, layout: u32) -> Self {
        assert!(keymap.num_layouts() > layout);

        let key_rows = Setup { numpad: false }.key_rows();

        let mut rows = Vec::new();
        for key_row in key_rows.iter() {
            let mut row = Vec::with_capacity(key_row.len());
            for &keyname in key_row.iter() {
                let width = match keyname {
                    "BKSL" => 1.5,
                    "BKSP" => 2.0,
                    "DELE" => 2.0,
                    "CAPS" => 1.75,
                    "LALT" => 1.25,
                    "LCTL" => 1.25,
                    "LFSH" => 2.25,
                    "LWIN" => 1.25,
                    "RALT" => 1.25,
                    "RCTL" => 1.25,
                    "RTSH" => 1.75,
                    "RTRN" => 2.25,
                    "RWIN" => 1.25,
                    "SPCE" => 5.5,
                    "TAB" => 1.5,
                    _ => 1.0,
                };

                let gamepad_mapping = match keyname {
                    "BKSP" => Some(gilrs::Button::West),
                    "CAPS" => Some(gilrs::Button::LeftThumb),
                    "LFSH" => Some(gilrs::Button::LeftTrigger2),
                    "RTRN" => Some(gilrs::Button::RightTrigger2),
                    "SPCE" => Some(gilrs::Button::North),
                    _ => None,
                };

                let mut key = Key {
                    id: widget::Id::unique(),
                    levels: vec![KeyLevel::for_name(keyname)],
                    width,
                    keycode: None,
                    gamepad_mapping,
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

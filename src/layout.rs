// SPDX-License-Identifier: GPL-3.0-only

use xkbcommon::xkb::{self, Keycode};

#[derive(Clone, Copy, Debug)]
pub enum Action {
    None,
    Keycode(xkb::Keycode),
}

#[derive(Clone, Debug)]
pub struct Key {
    pub name: String,
    pub width: f32,
    pub action: Action,
}

#[derive(Clone, Debug, Default)]
pub struct Layer {
    pub rows: Vec<Vec<Key>>,
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub layers: Vec<Layer>,
}

impl From<&xkb::Keymap> for Layout {
    fn from(keymap: &xkb::Keymap) -> Self {
        if keymap.num_layouts() == 0 {
            return Layout::default();
        }

        let key_rows: &'static [&'static [&'static str]] = &[
            &[
                "ESC", "FK01", "FK02", "FK03", "FK04", "FK05", "FK06", "FK07", "FK08", "FK09",
                "FK10", "FK11", "FK12", "DELE", "HOME",
            ],
            &[
                "TLDE", "AE01", "AE02", "AE03", "AE04", "AE05", "AE06", "AE07", "AE08", "AE09",
                "AE10", "AE11", "AE12", "BKSP", "PGUP",
            ],
            &[
                "TAB", "AD01", "AD02", "AD03", "AD04", "AD05", "AD06", "AD07", "AD08", "AD09",
                "AD10", "AD11", "AD12", "BKSL", "PGDN",
            ],
            &[
                "CAPS", "AC01", "AC02", "AC03", "AC04", "AC05", "AC06", "AC07", "AC08", "AC09",
                "AC10", "AC11", "RTRN", "END",
            ],
            &[
                "LFSH", "AB01", "AB02", "AB03", "AB04", "AB05", "AB06", "AB07", "AB08", "AB09",
                "AB10", "RTSH", "UP", "INS",
            ],
            &[
                "LCTL", "LALT", "LWIN", "SPCE", "RALT", "RWIN", "RCTL", "LEFT", "DOWN", "RGHT",
            ],
        ];

        let mut normal_layer = Layer::default();
        let mut shift_layer = Layer::default();
        for key_row in key_rows.iter() {
            let mut normal_row = Vec::with_capacity(key_row.len());
            let mut shift_row = Vec::with_capacity(key_row.len());
            for &key in key_row.iter() {
                let mut normal_key = Key {
                    name: key.to_string(),
                    width: 1.0,
                    action: Action::None,
                };
                let mut shift_key = Key {
                    name: key.to_string(),
                    width: 1.0,
                    action: Action::None,
                };

                match keymap.key_by_name(key) {
                    Some(kc) => {
                        normal_key.action = Action::Keycode(kc);
                        shift_key.action = Action::Keycode(kc);

                        let normal_syms = keymap.key_get_syms_by_level(kc, 0, 0);
                        if let Some(normal_sym) = normal_syms.get(0) {
                            normal_key.name = xkb::keysym_get_name(*normal_sym);
                            if let Some(normal_char) = normal_sym.key_char() {
                                if !normal_char.is_control() {
                                    normal_key.name = normal_char.to_string();
                                }
                            }

                            // Copy normal key name over by default
                            shift_key.name = normal_key.name.clone();
                        }

                        let shift_syms = keymap.key_get_syms_by_level(kc, 0, 1);
                        // let t = keymap.key_get_sysms_by;
                        if let Some(shift_sym) = shift_syms.get(0) {
                            shift_key.name = xkb::keysym_get_name(*shift_sym);
                            if let Some(shift_char) = shift_sym.key_char() {
                                if !shift_char.is_control() {
                                    shift_key.name = shift_char.to_string();
                                }
                            }
                        }
                    }
                    None => {
                        eprintln!("cannot find keycode for {:?} in keymap", key);
                    }
                }

                let name_width = match key {
                    "BKSL" => {
                        normal_key.width = 1.5;
                        shift_key.width = 1.5;
                        None
                    }
                    "BKSP" => Some(("Bksp", 2.0)),
                    "DELE" => Some(("Del", 2.0)),
                    "CAPS" => Some(("Caps", 1.75)),
                    "ESC" => Some(("Esc", 1.0)),
                    "LALT" => Some(("Alt", 1.25)),
                    "LCTL" => Some(("Ctrl", 1.25)),
                    "LFSH" => Some(("Shift", 2.25)),
                    "LWIN" => Some(("Super", 1.25)),
                    "PGDN" => Some(("PgDn", 1.0)),
                    "PGUP" => Some(("PgUp", 1.0)),
                    "RALT" => Some(("Alt", 1.25)),
                    "RCTL" => Some(("Ctrl", 1.25)),
                    "RTSH" => Some(("Shift", 1.75)),
                    "RTRN" => Some(("Enter", 2.25)),
                    "RWIN" => Some(("Super", 1.25)),
                    "SPCE" => Some((" ", 5.5)),
                    "TAB" => Some(("Tab", 1.5)),
                    _ => None,
                };
                if let Some((name, width)) = name_width {
                    normal_key.name = name.to_string();
                    normal_key.width = width;

                    shift_key.name = name.to_string();
                    shift_key.width = width;
                }

                normal_row.push(normal_key);
                shift_row.push(shift_key);
            }
            normal_layer.rows.push(normal_row);
            shift_layer.rows.push(shift_row);
        }
        Layout {
            layers: vec![normal_layer, shift_layer],
        }
    }
}

impl Layout {
    pub fn get_keycode(&self, name: &str) -> Option<&Keycode> {
        let result: Option<&xkb::Keycode> = self.layers[0]
            .rows
            .iter()
            .chain(self.layers[1].rows.iter())
            .flatten()
            .find(|key| key.name == name)
            .and_then(|key| match &key.action {
                Action::Keycode(kc) => Some(kc),
                Action::None => None,
            });

        result
    }
}

// SPDX-License-Identifier: GPL-3.0-only

use xkbcommon::xkb;

#[derive(Clone, Debug)]
pub enum Action {
    None,
    Keycode(xkb::Keycode),
    Layer(usize),
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
                "TLDE", "AE01", "AE02", "AE03", "AE04", "AE05", "AE06", "AE07", "AE08", "AE09",
                "AE10", "AE11", "AE12", "BKSP",
            ],
            &[
                "TAB", "AD01", "AD02", "AD03", "AD04", "AD05", "AD06", "AD07", "AD08", "AD09",
                "AD10", "AD11", "AD12", "BKSL",
            ],
            &[
                "CAPS", "AC01", "AC02", "AC03", "AC04", "AC05", "AC06", "AC07", "AC08", "AC09",
                "AC10", "AC11", "RTRN",
            ],
            &[
                "LFSH", "AB01", "AB02", "AB03", "AB04", "AB05", "AB06", "AB07", "AB08", "AB09",
                "AB10", "RTSH",
            ],
            &["LCTL", "LALT", "LWIN", "SPCE", "RALT", "RCTL"],
        ];

        let mut normal_layer = Layer::default();
        let mut shift_layer = Layer::default();
        for key_row in key_rows.iter() {
            let mut normal_row = Vec::with_capacity(key_row.len());
            let mut shift_row = Vec::with_capacity(key_row.len());
            for &key in key_row.iter() {
                let mut normal_key = Key {
                    name: key.to_lowercase(),
                    width: 1.0,
                    action: Action::None,
                };
                let mut shift_key = Key {
                    name: key.to_uppercase(),
                    width: 1.0,
                    action: Action::None,
                };

                match keymap.key_by_name(key) {
                    Some(kc) => {
                        normal_key.action = Action::Keycode(kc);
                        //TODO: actually trigger shift
                        shift_key.action = Action::Keycode(kc);

                        let normal_syms = keymap.key_get_syms_by_level(kc, 0, 0);
                        if let Some(normal_sym) = normal_syms.get(0) {
                            if let Some(normal_char) = normal_sym.key_char() {
                                normal_key.name = normal_char.to_string();
                            }
                        }

                        let shift_syms = keymap.key_get_syms_by_level(kc, 0, 1);
                        if let Some(shift_sym) = shift_syms.get(0) {
                            if let Some(shift_char) = shift_sym.key_char() {
                                shift_key.name = shift_char.to_string();
                            }
                        }
                    }
                    None => {
                        eprintln!("cannot find keycode for {:?} in keymap", key);
                    }
                }

                match key {
                    "LFSH" => {
                        //TODO: actually set modifier
                        normal_key.name = "⇧".to_string();
                        normal_key.width = 1.5;
                        normal_key.action = Action::Layer(1);
                        shift_key.name = "⇩".to_string();
                        shift_key.width = 1.5;
                        shift_key.action = Action::Layer(0);
                    }
                    "BKSP" => {
                        normal_key.name = "⌫".to_string();
                        normal_key.width = 1.5;
                        shift_key.name = "⌫".to_string();
                        shift_key.width = 1.5;
                    }
                    "SPCE" => {
                        normal_key.name = " ".to_string();
                        normal_key.width = 4.0;
                        shift_key.name = " ".to_string();
                        shift_key.width = 4.0;
                    }
                    _ => {}
                };

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

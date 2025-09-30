// SPDX-License-Identifier: GPL-3.0-only

const FULL_KEY_ROWS: &'static [&'static [(&'static str, u8)]] = &[
    &[
        ("ESC", 8),
        ("FK01", 8),
        ("FK02", 8),
        ("FK03", 8),
        ("FK04", 8),
        ("FK05", 8),
        ("FK06", 8),
        ("FK07", 8),
        ("FK08", 8),
        ("FK09", 8),
        ("FK10", 8),
        ("FK11", 8),
        ("FK12", 8),
        ("DELE", 16),
        ("HOME", 8),
    ],
    &[
        ("TLDE", 8),
        ("AE01", 8),
        ("AE02", 8),
        ("AE03", 8),
        ("AE04", 8),
        ("AE05", 8),
        ("AE06", 8),
        ("AE07", 8),
        ("AE08", 8),
        ("AE09", 8),
        ("AE10", 8),
        ("AE11", 8),
        ("AE12", 8),
        ("BKSP", 16),
        ("PGUP", 8),
    ],
    &[
        ("TAB", 12),
        ("AD01", 8),
        ("AD02", 8),
        ("AD03", 8),
        ("AD04", 8),
        ("AD05", 8),
        ("AD06", 8),
        ("AD07", 8),
        ("AD08", 8),
        ("AD09", 8),
        ("AD10", 8),
        ("AD11", 8),
        ("AD12", 8),
        ("BKSL", 12),
        ("PGDN", 8),
    ],
    &[
        ("CAPS", 14),
        ("AC01", 8),
        ("AC02", 8),
        ("AC03", 8),
        ("AC04", 8),
        ("AC05", 8),
        ("AC06", 8),
        ("AC07", 8),
        ("AC08", 8),
        ("AC09", 8),
        ("AC10", 8),
        ("AC11", 8),
        ("RTRN", 18),
        ("END", 8),
    ],
    &[
        ("LFSH", 18),
        ("AB01", 8),
        ("AB02", 8),
        ("AB03", 8),
        ("AB04", 8),
        ("AB05", 8),
        ("AB06", 8),
        ("AB07", 8),
        ("AB08", 8),
        ("AB09", 8),
        ("AB10", 8),
        ("RTSH", 14),
        ("UP", 8),
        ("INS", 8),
    ],
    &[
        ("LCTL", 10),
        ("LALT", 10),
        ("LWIN", 10),
        ("SPCE", 38),
        ("TGLLAYOUT", 8),
        ("RALT", 10),
        ("RWIN", 10),
        ("RCTL", 10),
        ("LEFT", 8),
        ("DOWN", 8),
        ("RGHT", 8),
    ],
];
const PARTIAL_KEY_ROWS: &'static [&'static [(&'static str, u8)]] = &[
    &[
        ("ESC", 6),
        ("AE01", 8),
        ("AE02", 8),
        ("AE03", 8),
        ("AE04", 8),
        ("AE05", 8),
        ("AE06", 8),
        ("AE07", 8),
        ("AE08", 8),
        ("AE09", 8),
        ("AE10", 8),
    ],
    &[
        ("AB10", 8),
        ("AD01", 8),
        ("AD02", 8),
        ("AD03", 8),
        ("AD04", 8),
        ("AD05", 8),
        ("AD06", 8),
        ("AD07", 8),
        ("AD08", 8),
        ("AD09", 8),
        ("AD10", 8),
        ("AB09", 8),
    ],
    &[
        ("TAB", 12),
        ("AC01", 8),
        ("AC02", 8),
        ("AC03", 8),
        ("AC04", 8),
        ("AC05", 8),
        ("AC06", 8),
        ("AC07", 8),
        ("AC08", 8),
        ("AC09", 8),
        ("AC11", 8),
        ("RTRN", 12),
    ],
    &[
        ("CAPS", 16),
        ("AB01", 8),
        ("AB02", 8),
        ("AB03", 8),
        ("AB04", 8),
        ("AB05", 8),
        ("AB06", 8),
        ("AB07", 8),
        ("AB08", 8),
        ("AB09", 8),
        ("UP", 8),
        ("AB10", 8),
    ],
    &[
        ("LCTL", 6),
        ("LALT", 2),
        ("LWIN", 2),
        ("SPCE", 8),
        ("TGLLAYOUT", 2),
        ("LEFT", 3),
        ("DOWN", 3),
        ("RGHT", 3),
    ],
];

use xkbcommon::xkb::{self, Keycode};

#[derive(Clone, Copy, Debug)]
pub enum Action {
    None,
    ToggleLayout,
    Keycode(xkb::Keycode),
}

#[derive(Clone, Debug)]
pub struct Key {
    pub name: String,
    pub width: u8,
    pub action: Action,
}

#[derive(Clone, Debug, Default)]
pub struct Layer {
    pub rows: Vec<Vec<Key>>,
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub full_layers: Vec<Layer>,
    // smaller
    pub partial_layers: Vec<Layer>,
}

impl From<&xkb::Keymap> for Layout {
    fn from(keymap: &xkb::Keymap) -> Self {
        if keymap.num_layouts() == 0 {
            return Layout::default();
        }

        let (full_normal_layer, full_shift_layer) = get_layers(keymap, FULL_KEY_ROWS);
        let (partial_normal_layer, partial_shift_layer) = get_layers(keymap, PARTIAL_KEY_ROWS);
        Layout {
            full_layers: vec![full_normal_layer, full_shift_layer],
            partial_layers: vec![partial_normal_layer, partial_shift_layer],
        }
    }
}

fn get_layers(keymap: &xkb::Keymap, rows: &[&[(&str, u8)]]) -> (Layer, Layer) {
    let mut normal_layer = Layer::default();
    let mut shift_layer = Layer::default();
    for key_row in rows.iter() {
        let mut normal_row = Vec::with_capacity(key_row.len());
        let mut shift_row = Vec::with_capacity(key_row.len());
        for (key, size) in key_row.iter() {
            let mut normal_key = Key {
                name: key.to_string(),
                width: *size,
                action: Action::None,
            };
            let mut shift_key = Key {
                name: key.to_string(),
                width: *size,
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
                    if key == &"TGLLAYOUT" {
                        normal_key.action = Action::ToggleLayout;
                        shift_key.action = Action::ToggleLayout;
                    } else {
                        eprintln!("cannot find keycode for {:?} in keymap", key);
                    }
                }
            }

            let name: Option<&str> = match *key {
                "BKSP" => Some("Bksp"),
                "DELE" => Some("Del"),
                "CAPS" => Some("Caps"),
                "ESC" => Some("Esc"),
                "LALT" => Some("Alt"),
                "LCTL" => Some("Ctrl"),
                "LFSH" => Some("Shift"),
                "LWIN" => Some("Super"),
                "PGDN" => Some("PgDn"),
                "PGUP" => Some("PgUp"),
                "RALT" => Some("Alt"),
                "RCTL" => Some("Ctrl"),
                "RTSH" => Some("Shift"),
                "RTRN" => Some("Enter"),
                "RWIN" => Some("Super"),
                "SPCE" => Some(" "),
                "TAB" => Some("Tab"),
                "TGLLAYOUT" => Some("<->"),
                _ => None,
            };

            if let Some(name) = name {
                normal_key.name = name.to_string();
                shift_key.name = name.to_string();
            }

            normal_row.push(normal_key);
            shift_row.push(shift_key);
        }
        normal_layer.rows.push(normal_row);
        shift_layer.rows.push(shift_row);
    }
    (normal_layer, shift_layer)
}

impl Layout {
    pub fn get_keycode(&self, name: &str) -> Option<&Keycode> {
        let result: Option<&xkb::Keycode> = self.full_layers[0]
            .rows
            .iter()
            .chain(self.full_layers[1].rows.iter())
            .flatten()
            .find(|key| key.name == name)
            .and_then(|key| match &key.action {
                Action::Keycode(kc) => Some(kc),
                _ => None,
            });

        result
    }
}

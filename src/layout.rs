#[derive(Clone, Debug)]
pub enum Action {
    Character,
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

impl Layout {
    //TODO: load from external data
    pub fn us() -> Self {
        let key_rows: &'static [&'static [&'static str]] = &[
            &["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
            &["a", "s", "d", "f", "g", "h", "j", "k", "l"],
            &["⇧", "z", "x", "c", "v", "b", "n", "m", "⌫"],
            &[",", " ", "."],
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
                    action: Action::Character,
                };
                let mut shift_key = Key {
                    name: key.to_uppercase(),
                    width: 1.0,
                    action: Action::Character,
                };
                match key {
                    "⇧" => {
                        normal_key.action = Action::Layer(1);
                        normal_key.width = 1.5;
                        shift_key.action = Action::Layer(0);
                        shift_key.width = 1.5;
                    }
                    "⌫" => {
                        normal_key.width = 1.5;
                        shift_key.width = 1.5;
                    }
                    " " => {
                        normal_key.width = 4.0;
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

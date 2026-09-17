// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    Element,
    app::Core,
    theme,
    widget::{
        self,
        menu::{self, key_bind::KeyBind},
    },
};
use std::collections::HashMap;

use crate::{Action, Config, Message, fl};

pub fn menu_bar<'a>(
    _core: &Core,
    _config: &Config,
    key_binds: &HashMap<KeyBind, Action>,
) -> Element<'a, Message> {
    menu::bar(vec![
        menu::Tree::with_children(
            widget::RcElementWrapper::new(
                widget::button::icon(widget::icon::from_name("input-keyboard-symbolic"))
                    .class(theme::Button::MenuRoot)
                    .into(),
            ),
            menu::items(
                key_binds,
                vec![
                    menu::Item::Button("Default", None, Action::None),
                    menu::Item::Button("Compact", None, Action::None),
                    menu::Item::Button("Mobile", None, Action::None),
                ],
            ),
        ),
        menu::Tree::with_children(
            widget::RcElementWrapper::new(
                widget::button::icon(widget::icon::from_name("view-more-symbolic"))
                    .class(theme::Button::MenuRoot)
                    .into(),
            ),
            menu::items(
                key_binds,
                vec![
                    menu::Item::CheckBox("System and function keys", None, false, Action::None),
                    menu::Item::CheckBox("Numpad", None, false, Action::None),
                    menu::Item::Divider,
                    //TODO: Zoom?
                    //TODO: Divider
                    menu::Item::Folder(
                        "Appearance",
                        vec![
                            menu::Item::CheckBox("Match desktop", None, false, Action::None),
                            menu::Item::CheckBox("Dark", None, false, Action::None),
                            menu::Item::CheckBox("Light", None, false, Action::None),
                        ],
                    ),
                    //TODO: Opacity
                    menu::Item::Divider,
                    menu::Item::Button("Settings...", None, Action::None),
                ],
            ),
        ),
    ])
    .item_height(menu::ItemHeight::Dynamic(40))
    .item_width(menu::ItemWidth::Uniform(320))
    .spacing(theme::active().cosmic().spacing.space_xxxs.into())
    .into()
}

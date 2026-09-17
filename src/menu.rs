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
use cosmic_osk_config::{AppTheme, Config};
use std::collections::HashMap;

use crate::{Action, Message, fl};

pub fn menu_bar<'a>(
    _core: &Core,
    config: &Config,
    key_binds: &HashMap<KeyBind, Action>,
) -> Element<'a, Message> {
    menu::bar(vec![
        /*TODO: compact and mobile modes
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
        */
        menu::Tree::with_children(
            widget::RcElementWrapper::new(
                widget::button::icon(widget::icon::from_name("view-more-symbolic"))
                    .class(theme::Button::MenuRoot)
                    .into(),
            ),
            menu::items(
                key_binds,
                vec![
                    menu::Item::CheckBox(
                        fl!("system-and-function-keys"),
                        None,
                        config.function_row,
                        Action::SetFunctionRow(!config.function_row),
                    ),
                    menu::Item::CheckBox(
                        fl!("numpad"),
                        None,
                        config.numpad,
                        Action::SetNumpad(!config.numpad),
                    ),
                    menu::Item::Divider,
                    //TODO: Zoom?
                    //TODO: Divider
                    menu::Item::Folder(
                        fl!("appearance"),
                        vec![
                            menu::Item::CheckBox(
                                fl!("appearance", "match-desktop"),
                                None,
                                config.app_theme == AppTheme::System,
                                Action::SetAppTheme(AppTheme::System),
                            ),
                            menu::Item::CheckBox(
                                fl!("appearance", "dark"),
                                None,
                                config.app_theme == AppTheme::Dark,
                                Action::SetAppTheme(AppTheme::Dark),
                            ),
                            menu::Item::CheckBox(
                                fl!("appearance", "light"),
                                None,
                                config.app_theme == AppTheme::Light,
                                Action::SetAppTheme(AppTheme::Light),
                            ),
                        ],
                    ),
                    //TODO: Opacity
                    menu::Item::Divider,
                    menu::Item::Button(fl!("settings"), None, Action::Settings),
                ],
            ),
        ),
    ])
    .item_height(menu::ItemHeight::Dynamic(40))
    .item_width(menu::ItemWidth::Uniform(320))
    .spacing(theme::active().cosmic().spacing.space_xxxs.into())
    .into()
}

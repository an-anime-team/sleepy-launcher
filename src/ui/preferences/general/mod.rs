// SPDX-License-Identifier: GPL-3.0-or-later
//
// sleepy-launcher
// Copyright (C) 2021 - 2026  Nikita Podvirnyi <krypt0nn@dawn.wine>
// Copyright (C) 2025 - 2026  @JohnTheCoolingFan <ivan8215145640@gmail.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use gtk::prelude::*;
use adw::prelude::*;

use anime_launcher_sdk::wincompatlib::prelude::*;

use anime_launcher_sdk::config::ConfigExt;
use anime_launcher_sdk::zzz::config::Config;
use anime_launcher_sdk::zzz::config::schema::prelude::*;

use anime_launcher_sdk::anime_game_core::zzz::consts::GameEdition;
use anime_launcher_sdk::zzz::env_emulation::Environment;

pub mod components;

use components::*;

use crate::i18n::*;

use crate::*;

use super::main::PreferencesAppMsg;

pub struct GeneralApp {
    components_page: AsyncController<ComponentsPage>,

    game_diff: Option<VersionDiff>,
    style: LauncherStyle,
    use_video_background: bool,
    background_index: u8,
    languages: Vec<String>
}

#[derive(Debug, Clone)]
pub enum GeneralAppMsg {
    /// Supposed to be called automatically on app's run when the latest game version
    /// was retrieved from the API
    SetGameDiff(Option<VersionDiff>),

    UpdateDownloadedWine,
    UpdateDownloadedDxvk,

    RepairGame,
    RemakePrefix,

    OpenMainPage,
    OpenComponentsPage,

    UpdateLauncherStyle(LauncherStyle),
    SetVideoBackground(bool),

    WineOpen(&'static [&'static str]),

    Toast {
        title: String,
        description: Option<String>
    }
}

#[relm4::component(async, pub)]
impl SimpleAsyncComponent for GeneralApp {
    type Init = ();
    type Input = GeneralAppMsg;
    type Output = PreferencesAppMsg;

    view! {
        #[root]
        adw::PreferencesPage {
            set_title: &i18n!("general"),
            set_icon_name: Some("applications-system-symbolic"),

            add = &adw::PreferencesGroup {
                set_title: &i18n!("appearance"),

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::Center,

                    set_spacing: 32,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,

                        gtk::ToggleButton {
                            add_css_class: "card",

                            set_width_request: 180,
                            set_height_request: 120,

                            #[watch]
                            set_active: model.style == LauncherStyle::Modern,

                            gtk::Picture {
                                set_resource: Some(&format!("{APP_RESOURCE_PATH}/images/modern.svg")),
                                set_content_fit: gtk::ContentFit::ScaleDown
                            },

                            connect_clicked => GeneralAppMsg::UpdateLauncherStyle(LauncherStyle::Modern)
                        },

                        gtk::Label {
                            set_text: &i18n!("modern"),

                            set_margin_top: 16
                        }
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,

                        gtk::ToggleButton {
                            add_css_class: "card",

                            set_width_request: 180,
                            set_height_request: 120,

                            #[watch]
                            set_active: model.style == LauncherStyle::Classic,

                            gtk::Picture {
                                set_resource: Some(&format!("{APP_RESOURCE_PATH}/images/classic.svg")),
                                set_content_fit: gtk::ContentFit::ScaleDown
                            },

                            connect_clicked => GeneralAppMsg::UpdateLauncherStyle(LauncherStyle::Classic)
                        },

                        gtk::Label {
                            set_text: &i18n!("classic"),

                            set_margin_top: 16
                        }
                    }
                }
            },

            add = &adw::PreferencesGroup {
                #[watch]
                set_visible: model.style == LauncherStyle::Classic,

                adw::ActionRow {
                    set_title: &i18n!("update-background"),
                    set_subtitle: &i18n!("update-background-description"),

                    add_suffix = &gtk::Switch {
                        set_valign: gtk::Align::Center,
                        set_active: !KEEP_BACKGROUND_FILE.exists(),

                        connect_state_notify => |switch| {
                            #[allow(unused_must_use)]
                            if switch.is_active() {
                                std::fs::remove_file(KEEP_BACKGROUND_FILE.as_path());
                            } else {
                                std::fs::write(KEEP_BACKGROUND_FILE.as_path(), "");
                            }
                        }
                    }
                },

                adw::ActionRow {
                    set_title: &i18n!("video-background"),
                    set_subtitle: &i18n!("video-background-description"),

                    add_suffix = &gtk::Switch {
                        set_valign: gtk::Align::Center,
                        set_active: model.use_video_background,

                        connect_state_notify[sender] => move |switch| {
                            sender.input(GeneralAppMsg::SetVideoBackground(switch.state()));
                        }
                    }
                }
            },

            add = &adw::PreferencesGroup {
                set_title: &i18n!("general"),

                adw::ComboRow {
                    set_title: &i18n!("launcher-language"),
                    set_subtitle: &i18n!("launcher-language-description"),

                    set_model: Some(&gtk::StringList::new(&model.languages.iter().map(|lang| lang.as_str()).collect::<Vec<&str>>())),

                    set_selected: {
                        let selected = crate::i18n::get_lang().language;

                        SUPPORTED_LANGUAGES.iter()
                            .position(|lang| lang.language == selected)
                            .unwrap_or(0) as u32
                    },

                    connect_selected_notify => |row| {
                        if is_ready() && let Ok(mut config) = Config::get() {
                            config.launcher.language = crate::i18n::format_lang(SUPPORTED_LANGUAGES
                                .get(row.selected() as usize)
                                .unwrap_or(&SUPPORTED_LANGUAGES[0]));

                            Config::update(config);
                        }
                    }
                },

                adw::ComboRow {
                    set_title: &i18n!("game-edition"),

                    set_model: Some(&gtk::StringList::new(&[
                        &i18n!("global"),
                        &i18n!("china")
                    ])),

                    set_selected: GameEdition::list().iter()
                        .position(|edition| edition == &CONFIG.launcher.edition)
                        .unwrap() as u32,

                    connect_selected_notify[sender] => move |row| {
                        if is_ready() {
                            #[allow(unused_must_use)]
                            if let Ok(mut config) = Config::get() {
                                config.launcher.edition = GameEdition::list()[row.selected() as usize];

                                Config::update(config);

                                sender.output(PreferencesAppMsg::UpdateLauncherState);
                            }
                        }
                    }
                },

                adw::ComboRow {
                    set_title: &i18n!("game-environment"),
                    set_subtitle: &i18n!("game-environment-description"),

                    set_model: Some(&gtk::StringList::new(&[
                        "Hoyoverse",
                        "Google Play"
                    ])),

                    set_selected: match CONFIG.launcher.environment {
                        Environment::PC => 0,
                        Environment::Android => 1,

                        _ => unreachable!()
                    },

                    connect_selected_notify => |row| {
                        if is_ready() && let Ok(mut config) = Config::get() {
                            config.launcher.environment = match row.selected() {
                                0 => Environment::PC,
                                1 => Environment::Android,

                                _ => unreachable!()
                            };

                            Config::update(config);
                        }
                    }
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 8,
                    set_margin_top: 16,

                    gtk::Button {
                        set_label: &i18n!("repair-game"),

                        connect_clicked => GeneralAppMsg::RepairGame
                    },

                    gtk::Button {
                        set_label: &i18n!("remake-prefix"),

                        add_css_class: "destructive-action",

                        connect_clicked => GeneralAppMsg::RemakePrefix
                    }
                }
            },

            add = &adw::PreferencesGroup {
                set_title: &i18n!("status"),

                adw::ActionRow {
                    set_title: &i18n!("game-version"),

                    add_suffix = &gtk::Label {
                        #[watch]
                        set_text: &match model.game_diff.as_ref() {
                            Some(diff) => match diff {
                                VersionDiff::Latest { version: current, .. } |
                                VersionDiff::Predownload { current, .. } |
                                VersionDiff::Diff { current, .. } |
                                VersionDiff::Outdated { current, .. } => current.to_string(),

                                VersionDiff::NotInstalled { .. } => i18n!("game-not-installed")
                            }

                            None => String::from("?")
                        },

                        #[watch]
                        set_css_classes: match model.game_diff.as_ref() {
                            Some(diff) => match diff {
                                VersionDiff::Latest { .. }       => &["success"],
                                VersionDiff::Predownload { .. }  => &["accent"],
                                VersionDiff::Diff { .. }         => &["warning"],
                                VersionDiff::Outdated { .. }     => &["error"],
                                VersionDiff::NotInstalled { .. } => &[]
                            }

                            None => &[]
                        },

                        #[watch]
                        set_tooltip_text: Some(&match model.game_diff.as_ref() {
                            Some(diff) => match diff {
                                VersionDiff::Latest { .. } => String::new(),

                                VersionDiff::Predownload { current, latest, .. } => i18n!("game-predownload-available", {
                                    "old" => current.to_string(),
                                    "new" => latest.to_string()
                                }),

                                VersionDiff::Diff { current, latest, .. } => i18n!("game-update-available", {
                                    "old" => current.to_string(),
                                    "new" => latest.to_string()
                                }),

                                VersionDiff::Outdated { latest, ..} => i18n!("game-outdated", {
                                    "latest" => latest.to_string()
                                }),

                                VersionDiff::NotInstalled { .. } => String::new()
                            }

                            None => String::new()
                        })
                    }
                }
            },

            add = &adw::PreferencesGroup {
                set_title: &i18n!("options"),

                adw::ComboRow {
                    set_title: &i18n!("launcher-behavior"),
                    set_subtitle: &i18n!("launcher-behavior-description"),

                    set_model: Some(&gtk::StringList::new(&[
                        &i18n!("nothing"),
                        &i18n!("hide-verb"),
                        &i18n!("close-verb"),
                    ])),

                    set_selected: match CONFIG.launcher.behavior {
                        LauncherBehavior::Nothing => 0,
                        LauncherBehavior::Hide    => 1,
                        LauncherBehavior::Close   => 2
                    },

                    connect_selected_notify => |row| {
                        if is_ready() && let Ok(mut config) = Config::get() {
                            config.launcher.behavior = [
                                LauncherBehavior::Nothing,
                                LauncherBehavior::Hide,
                                LauncherBehavior::Close
                            ][row.selected() as usize];

                            Config::update(config);
                        }
                    }
                }
            },

            add = &adw::PreferencesGroup {
                adw::ActionRow {
                    set_title: &i18n!("components"),
                    set_subtitle: &i18n!("components-description"),

                    add_suffix = &gtk::Image {
                        set_icon_name: Some("go-next-symbolic")
                    },

                    set_activatable: true,

                    connect_activated => GeneralAppMsg::OpenComponentsPage
                },

                adw::ExpanderRow {
                    set_title: &i18n!("wine-tools"),

                    add_row = &adw::ActionRow {
                        set_title: &i18n!("command-line"),
                        set_subtitle: "wineconsole",

                        set_activatable: true,

                        connect_activated => GeneralAppMsg::WineOpen(&["wineconsole"])
                    },

                    add_row = &adw::ActionRow {
                        set_title: &i18n!("registry-editor"),
                        set_subtitle: "regedit",

                        set_activatable: true,

                        connect_activated => GeneralAppMsg::WineOpen(&["regedit"])
                    },

                    add_row = &adw::ActionRow {
                        set_title: &i18n!("explorer"),
                        set_subtitle: "explorer",

                        set_activatable: true,

                        connect_activated => GeneralAppMsg::WineOpen(&["explorer"])
                    },

                    add_row = &adw::ActionRow {
                        set_title: &i18n!("task-manager"),
                        set_subtitle: "taskmgr",

                        set_activatable: true,

                        connect_activated => GeneralAppMsg::WineOpen(&["taskmgr"])
                    },

                    add_row = &adw::ActionRow {
                        set_title: &i18n!("configuration"),
                        set_subtitle: "winecfg",

                        set_activatable: true,

                        connect_activated => GeneralAppMsg::WineOpen(&["winecfg"])
                    },

                    add_row = &adw::ActionRow {
                        set_title: &i18n!("debugger"),
                        set_subtitle: "start winedbg",

                        set_activatable: true,

                        connect_activated => GeneralAppMsg::WineOpen(&["start", "winedbg"])
                    }
                }
            }
        },

        #[local_ref]
        components_page -> adw::NavigationPage,
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        tracing::info!("Initializing general settings");

        let model = Self {
            components_page: ComponentsPage::builder()
                .launch(())
                .forward(sender.input_sender(), std::convert::identity),

            game_diff: None,
            style: CONFIG.launcher.style,
            use_video_background: CONFIG.launcher.video_background,
            background_index: CONFIG.launcher.background_index,
            languages: SUPPORTED_LANGUAGES.iter().map(|lang| i18n!(format_lang(lang).as_str())).collect()
        };

        let components_page = model.components_page.widget();

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
        tracing::debug!("Called general settings event: {:?}", msg);

        match msg {
            GeneralAppMsg::SetGameDiff(diff) => {
                self.game_diff = diff;
            }

            GeneralAppMsg::UpdateDownloadedWine => {
                self.components_page.sender()
                    .send(ComponentsPageMsg::UpdateDownloadedWine)
                    .unwrap();
            }

            GeneralAppMsg::UpdateDownloadedDxvk => {
                self.components_page.sender()
                    .send(ComponentsPageMsg::UpdateDownloadedDxvk)
                    .unwrap();
            }

            GeneralAppMsg::RepairGame => {
                sender.output(Self::Output::RepairGame).unwrap();
            }

            GeneralAppMsg::RemakePrefix => {
                sender.output(Self::Output::RemakePrefix).unwrap();
            }

            GeneralAppMsg::OpenMainPage => unsafe {
                // I honestly don't care anymore.
                #[allow(static_mut_refs)]
                PREFERENCES_WINDOW.as_ref()
                    .unwrap_unchecked()
                    .widget()
                    .pop_subpage();
            }

            GeneralAppMsg::OpenComponentsPage => unsafe {
                // I honestly don't care anymore.
                #[allow(static_mut_refs)]
                PREFERENCES_WINDOW.as_ref()
                    .unwrap_unchecked()
                    .widget()
                    .push_subpage(self.components_page.widget());
            }

            #[allow(unused_must_use)]
            GeneralAppMsg::UpdateLauncherStyle(style) => {
                if style == LauncherStyle::Classic && !KEEP_BACKGROUND_FILE.exists()
                    && let Err(err) = crate::background::download_background(
                        self.use_video_background,
                        self.background_index
                    )
                {
                    tracing::error!("Failed to download background picture/video");

                    sender.input(GeneralAppMsg::Toast {
                        title: i18n!("background-downloading-failed"),
                        description: Some(err.to_string())
                    });

                    return;
                }

                if let Ok(mut config) = Config::get() {
                    config.launcher.style = style;

                    Config::update(config);
                }

                self.style = style;

                sender.output(Self::Output::SetLauncherStyle(style));
            }

            GeneralAppMsg::SetVideoBackground(use_video) => {
                if self.style == LauncherStyle::Classic
                    && !KEEP_BACKGROUND_FILE.exists() && use_video
                    && let Err(err) = crate::background::download_background(
                        use_video,
                        self.background_index
                    )
                {
                    tracing::error!("Failed to download background picture/video");

                    sender.input(GeneralAppMsg::Toast {
                        title: i18n!("background-downloading-failed"),
                        description: Some(err.to_string())
                    });

                    return;
                }

                if let Ok(mut config) = Config::get() {
                    config.launcher.video_background = use_video;

                    Config::update(config);
                }

                self.use_video_background = use_video;

                let _ = sender.output(Self::Output::SetVideoBackground(use_video));
            }

            GeneralAppMsg::WineOpen(executable) => {
                let config = Config::get().unwrap_or_else(|_| CONFIG.clone());

                if let Ok(Some(wine)) = config.get_selected_wine() {
                    let result = wine
                        .to_wine(config.components.path, Some(config.game.wine.builds.join(&wine.name)))
                        .with_prefix(config.game.wine.prefix)
                        .with_loader(WineLoader::Current)
                        .with_arch(WineArch::Win64)
                        .run_args(executable);

                    if let Err(err) = result {
                        sender.input(GeneralAppMsg::Toast {
                            title: i18n!("wine-run-error", {
                                "executable" => executable.join(" ")
                            }),
                            description: Some(err.to_string())
                        });

                        tracing::error!("Failed to run {:?} using wine: {err}", executable);
                    }
                }
            }

            #[allow(unused_must_use)]
            GeneralAppMsg::Toast { title, description } => {
                sender.output(Self::Output::Toast { title, description });
            }
        }
    }
}

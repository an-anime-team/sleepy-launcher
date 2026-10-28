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

use anime_launcher_sdk::anime_game_core::sophon_tools::region::SophonRegion;
use anime_launcher_sdk::zzz::env_emulation::Environment;

pub mod components;

use components::*;

use crate::i18n::*;

use crate::*;

use super::main::PreferencesAppMsg;

pub struct GeneralApp {
    components_page: AsyncController<ComponentsPage>,

    style: LauncherStyle,
    use_video_background: bool,
    background_index: u8,
    languages: Vec<String>,

    game_version: String
}

#[derive(Debug, Clone)]
pub enum GeneralAppMsg {
    UpdateDownloadedWine,
    UpdateDownloadedDxvk,

    SetGameVersion(Option<String>),
    UpdateGameVersion,
    ReloadComponents,

    RepairGame,
    RemakePrefix,

    OpenMainPage,
    OpenComponentsPage,

    UpdateLauncherStyle(LauncherStyle),
    SetVideoBackground(bool),

    /// Internal: apply launcher style after the background picture is ready.
    ApplyLauncherStyle(LauncherStyle),

    /// Internal: apply video background setting after the background is ready.
    ApplyVideoBackground(bool),

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

                    set_selected: match CONFIG.launcher.edition {
                        SophonRegion::Global => 0,
                        SophonRegion::China  => 1
                    },

                    connect_selected_notify[sender] => move |row| {
                        if is_ready() {
                            #[allow(unused_must_use)]
                            if let Ok(mut config) = Config::get() {
                                config.launcher.edition = match row.selected() {
                                    0 => SophonRegion::Global,
                                    _ => SophonRegion::China
                                };

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

            // TODO: game version status requires the new game diff API.
            add = &adw::PreferencesGroup {
                set_title: &i18n!("status"),

                adw::ActionRow {
                    set_title: &i18n!("game-version"),

                    add_suffix = &gtk::Label {
                        #[watch]
                        set_text: &model.game_version
                    }
                }
            },

            add = &adw::PreferencesGroup {
                set_title: &i18n!("game-downloader"),

                adw::SpinRow {
                    set_title: &i18n!("sophon_tools_settings_target_memory_usage_title"),
                    set_subtitle: &i18n!("sophon_tools_settings_target_memory_usage_description"),

                    set_digits: 0,
                    set_numeric: true,
                    set_climb_rate: 64.0,

                    set_adjustment: Some(&gtk::Adjustment::new(
                        CONFIG.launcher.downloader.target_memory_usage as f64,
                        0.0,
                        65536.0,
                        64.0,
                        512.0,
                        0.0
                    )),

                    connect_value_notify => |row| {
                        if is_ready() && let Ok(mut config) = Config::get() {
                            config.launcher.downloader.target_memory_usage = row.value() as u64;

                            Config::update(config);
                        }
                    }
                },

                adw::SpinRow {
                    set_title: &i18n!("sophon_tools_settings_max_parallel_streams_title"),
                    set_subtitle: &i18n!("sophon_tools_settings_max_parallel_streams_description"),

                    set_digits: 0,
                    set_numeric: true,
                    set_climb_rate: 1.0,

                    set_adjustment: Some(&gtk::Adjustment::new(
                        CONFIG.launcher.downloader.max_parallel_streams as f64,
                        0.0,
                        1024.0,
                        1.0,
                        16.0,
                        0.0
                    )),

                    connect_value_notify => |row| {
                        if is_ready() && let Ok(mut config) = Config::get() {
                            config.launcher.downloader.max_parallel_streams = row.value() as u16;

                            Config::update(config);
                        }
                    }
                },

                adw::SwitchRow {
                    set_title: &i18n!("sophon_tools_settings_fast_verify_title"),
                    set_subtitle: &i18n!("sophon_tools_settings_fast_verify_description"),

                    set_active: CONFIG.launcher.downloader.fast_verify,

                    connect_active_notify => |row| {
                        if is_ready() && let Ok(mut config) = Config::get() {
                            config.launcher.downloader.fast_verify = row.is_active();

                            Config::update(config);
                        }
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

            style: CONFIG.launcher.style,
            use_video_background: CONFIG.launcher.video_background,
            background_index: CONFIG.launcher.background_index,
            languages: SUPPORTED_LANGUAGES.iter().map(|lang| i18n!(format_lang(lang).as_str())).collect(),

            game_version: String::from("?")
        };

        let components_page = model.components_page.widget();

        let widgets = view_output!();

        Self::update_game_version(sender.clone());

        AsyncComponentParts { model, widgets }
    }

    async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
        tracing::debug!("Called general settings event: {:?}", msg);

        match msg {
            GeneralAppMsg::UpdateDownloadedWine => {
                let _ = self.components_page.sender()
                    .send(ComponentsPageMsg::UpdateDownloadedWine);
            }

            GeneralAppMsg::SetGameVersion(version) => {
                self.game_version = version
                    .unwrap_or_else(|| i18n!("game-not-installed"));
            }

            GeneralAppMsg::UpdateGameVersion => {
                Self::update_game_version(sender);
            }

            GeneralAppMsg::ReloadComponents => {
                let _ = self.components_page.sender()
                    .send(ComponentsPageMsg::Reload);
            }

            GeneralAppMsg::UpdateDownloadedDxvk => {
                let _ = self.components_page.sender()
                    .send(ComponentsPageMsg::UpdateDownloadedDxvk);
            }

            GeneralAppMsg::RepairGame => {
                let _ = sender.output(Self::Output::RepairGame);
            }

            GeneralAppMsg::RemakePrefix => {
                let _ = sender.output(Self::Output::RemakePrefix);
            }

            #[allow(static_mut_refs)]
            GeneralAppMsg::OpenMainPage => {
                if let Some(preferences) = unsafe { PREFERENCES_WINDOW.as_ref() } {
                    preferences.widget().pop_subpage();
                }
            }

            #[allow(static_mut_refs)]
            GeneralAppMsg::OpenComponentsPage => {
                if let Some(preferences) = unsafe { PREFERENCES_WINDOW.as_ref() } {
                    preferences.widget().push_subpage(self.components_page.widget());
                }
            }

            #[allow(unused_must_use)]
            GeneralAppMsg::UpdateLauncherStyle(style) => {
                if style == LauncherStyle::Classic && !KEEP_BACKGROUND_FILE.exists() {
                    let use_video = self.use_video_background;
                    let background_index = self.background_index;
                    let sender = sender.clone();

                    relm4::spawn(async move {
                        match crate::background::download_background(
                            use_video,
                            background_index
                        ).await {
                            Ok(()) => sender.input(GeneralAppMsg::ApplyLauncherStyle(style)),

                            Err(err) => {
                                tracing::error!("Failed to download background picture/video");

                                sender.input(GeneralAppMsg::Toast {
                                    title: i18n!("background-downloading-failed"),
                                    description: Some(err.to_string())
                                });
                            }
                        }
                    });

                    return;
                }

                self.apply_launcher_style(style, &sender);
            }

            GeneralAppMsg::ApplyLauncherStyle(style) => {
                self.apply_launcher_style(style, &sender);
            }

            GeneralAppMsg::SetVideoBackground(use_video) => {
                if self.style == LauncherStyle::Classic
                    && !KEEP_BACKGROUND_FILE.exists() && use_video
                {
                    let background_index = self.background_index;
                    let sender = sender.clone();

                    relm4::spawn(async move {
                        match crate::background::download_background(
                            use_video,
                            background_index
                        ).await {
                            Ok(()) => sender.input(GeneralAppMsg::ApplyVideoBackground(use_video)),

                            Err(err) => {
                                tracing::error!("Failed to download background picture/video");

                                sender.input(GeneralAppMsg::Toast {
                                    title: i18n!("background-downloading-failed"),
                                    description: Some(err.to_string())
                                });
                            }
                        }
                    });

                    return;
                }

                self.apply_video_background(use_video, &sender);
            }

            GeneralAppMsg::ApplyVideoBackground(use_video) => {
                self.apply_video_background(use_video, &sender);
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

impl GeneralApp {
    fn apply_launcher_style(
        &mut self,
        style: LauncherStyle,
        sender: &AsyncComponentSender<Self>
    ) {
        if let Ok(mut config) = Config::get() {
            config.launcher.style = style;

            Config::update(config);
        }

        self.style = style;

        let _ = sender.output(PreferencesAppMsg::SetLauncherStyle(style));
    }

    fn apply_video_background(
        &mut self,
        use_video: bool,
        sender: &AsyncComponentSender<Self>
    ) {
        if let Ok(mut config) = Config::get() {
            config.launcher.video_background = use_video;

            Config::update(config);
        }

        self.use_video_background = use_video;

        let _ = sender.output(PreferencesAppMsg::SetVideoBackground(use_video));
    }

    fn update_game_version(sender: AsyncComponentSender<Self>) {
        std::thread::spawn(move || {
            let Some(runtime) = crate::TOKIO_HANDLE.get().cloned() else {
                tracing::error!("Tokio runtime handle is not initialized");

                return;
            };

            let version = runtime.block_on(async {
                match crate::get_game_for_config().await {
                    Ok(game) => match game.detect_version().await {
                        Ok(version) => version,

                        Err(err) => {
                            tracing::error!("Failed to detect game version: {err}");

                            None
                        }
                    },

                    Err(err) => {
                        tracing::error!("Failed to load game info: {err}");

                        None
                    }
                }
            });

            sender.input(GeneralAppMsg::SetGameVersion(version));
        });
    }
}

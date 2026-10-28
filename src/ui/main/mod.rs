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

use relm4::actions::*;
use gtk::prelude::*;
use adw::prelude::*;
use gtk::glib::clone;

mod repair_game;
mod download_wine;
mod install_dxvk;
mod install_dx12;
mod download_diff;
mod create_prefix;
mod migrate_folder;
mod launch;
mod import_game;

use anime_launcher_sdk::components::loader::ComponentsLoader;
use anime_launcher_sdk::config::ConfigExt;
use anime_launcher_sdk::anime_game_core::game::GameDiff;
use anime_launcher_sdk::zzz::config::Config;
use anime_launcher_sdk::zzz::config::schema::launcher::LauncherStyle;
use anime_launcher_sdk::zzz::states::LauncherState;

use crate::*;
use crate::ui::components::*;
use super::preferences::main::*;
use super::about::*;

relm4::new_action_group!(WindowActionGroup, "win");

relm4::new_stateless_action!(LauncherFolder, WindowActionGroup, "launcher_folder");
relm4::new_stateless_action!(GameFolder, WindowActionGroup, "game_folder");
relm4::new_stateless_action!(ConfigFile, WindowActionGroup, "config_file");
relm4::new_stateless_action!(DebugFile, WindowActionGroup, "debug_file");

relm4::new_stateless_action!(SignalSearchUrl, WindowActionGroup, "signal_search_url");

relm4::new_stateless_action!(About, WindowActionGroup, "about");

pub static mut MAIN_WINDOW: Option<adw::ApplicationWindow> = None;
pub static mut PREFERENCES_WINDOW: Option<AsyncController<PreferencesApp>> = None;

pub struct App {
    progress_bar: AsyncController<ProgressBar>,

    toast_overlay: adw::ToastOverlay,

    game_loaded: bool,
    game_error: Option<String>,

    // The game itself is stored in the global `crate::GAMES` cache, so the
    // launcher state can safely borrow it for `LauncherState::GameDiff`.
    state: Option<LauncherState<'static>>,

    loading: Option<String>,
    style: LauncherStyle,
    use_video_background: bool,
    background_index: u8,

    downloading: bool,
    disabled_buttons: bool,
    kill_game_button: bool,
    disabled_kill_game_button: bool
}

// NOTE: `Game` and `LauncherState` don't implement `Debug` anymore, so this
//       enum can't derive it either. Keep manual formatting out of the logs.
pub enum AppMsg {
    UpdateLauncherState {
        /// Perform action when game or voice downloading is required
        /// Needed for chained executions (e.g. update one voice after another)
        perform_on_download_needed: bool,

        /// Show status gathering progress page
        show_status_page: bool
    },

    /// Supposed to be called automatically on app's run when the game info was
    /// retrieved from the API
    SetGameLoaded(bool),

    /// Supposed to be called automatically on app's run when the launcher state
    /// was chosen
    SetLauncherState(Option<LauncherState<'static>>),

    SetLauncherStyle(LauncherStyle),
    SetVideoBackground(bool),
    SetBackgroundIndex(u8),
    SetLoadingStatus(Option<String>),

    ReloadCss,

    UpdateGameVersion,
    ReloadComponents,

    SetDownloading(bool),
    DisableButtons(bool),
    SetKillGameButton(bool),
    DisableKillGameButton(bool),

    OpenPreferences,
    RepairGame,
    RemakePrefix,

    ImportGame,
    ImportGameFromPath(std::path::PathBuf),

    PerformAction,

    HideWindow,
    ShowWindow,

    SuggestTimeoutFix,

    Toast {
        title: String,
        description: Option<String>
    }
}

// `Game` and `LauncherState` don't implement `Debug`, so implement it manually
// to still satisfy Relm4's `Component` bounds.
impl std::fmt::Debug for AppMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppMsg").finish_non_exhaustive()
    }
}

#[relm4::component(pub, async)]
impl SimpleAsyncComponent for App {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    menu! {
        main_menu: {
            section! {
                &i18n!("launcher-folder") => LauncherFolder,
                &i18n!("game-folder") => GameFolder,
                &i18n!("config-file") => ConfigFile,
                &i18n!("debug-file") => DebugFile,
            },

            section! {
                &i18n!("signal-search-url") => SignalSearchUrl
            },

            section! {
                &i18n!("about") => About
            }
        }
    }

    view! {
        main_window = adw::ApplicationWindow {
            set_icon_name: Some(APP_ID),

            #[watch]
            set_default_size: (
                match model.style {
                    LauncherStyle::Modern => 900,
                    LauncherStyle::Classic => 1152 // (w = 2560 / 1440 * h, where 2560x1440 is default background picture resolution)
                },
                match model.style {
                    LauncherStyle::Modern => 600,
                    LauncherStyle::Classic => 648
                }
            ),

            #[watch]
            set_css_classes: &{
                let mut classes = vec!["background", "csd"];

                if APP_DEBUG {
                    classes.push("devel");
                }

                match model.style {
                    LauncherStyle::Modern => (),
                    LauncherStyle::Classic => {
                        if model.loading.is_none() {
                            classes.push("classic-style");
                        }
                    }
                }

                classes
            },

            #[local_ref]
            toast_overlay -> adw::ToastOverlay {
                gtk::Overlay {
                    #[name = "background_video"]
                    gtk::Picture::new() {
                        set_hexpand: true,
                        set_vexpand: true,

                        set_halign: gtk::Align::Fill,
                        set_valign: gtk::Align::Fill,
                        set_content_fit: gtk::ContentFit::Cover,

                        #[watch]
                        set_visible: {
                            model.style == LauncherStyle::Classic
                                && model.use_video_background
                                && model.loading.is_none()
                                && crate::BACKGROUND_VIDEO_FILE.exists()
                        },

                        #[wrap(Some)]
                        set_paintable = &gtk::MediaFile::for_filename(crate::BACKGROUND_VIDEO_FILE.as_path()) {
                            set_muted: true,
                            set_loop: true,

                            #[watch]
                            set_playing: {
                                !model.kill_game_button
                                    && model.style == LauncherStyle::Classic
                                    && model.use_video_background
                                    && model.loading.is_none()
                                    && crate::BACKGROUND_VIDEO_FILE.exists()
                            },

                            connect_error_notify: |mstream| {
                                if let Some(err) = mstream.error() {
                                    tracing::error!("Background video stream error: {err}");
                                }
                            }
                        }
                    },

                    add_overlay = &gtk::Box {
                        #[watch]
                        set_css_classes: {
                            if model.style == LauncherStyle::Classic
                                && model.use_video_background
                                && model.loading.is_none()
                            {
                                &["background-overlay"]
                            } else {
                                &[""]
                            }
                        }
                    },

                    #[name = "ui_contents"]
                    add_overlay = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,

                        adw::HeaderBar {
                            #[watch]
                            set_css_classes: match model.style {
                                LauncherStyle::Modern => &[""],
                                LauncherStyle::Classic => &["flat"]
                            },

                            #[wrap(Some)]
                            set_title_widget = &adw::WindowTitle {
                                #[watch]
                                set_title: match model.style {
                                    LauncherStyle::Modern => "Sleepy Launcher",
                                    LauncherStyle::Classic => ""
                                }
                            },

                            pack_end = &gtk::MenuButton {
                                set_icon_name: "open-menu-symbolic",
                                set_menu_model: Some(&main_menu)
                            }
                        },

                        adw::StatusPage {
                            set_title: &i18n!("loading-data"),
                            set_icon_name: Some(APP_ID),
                            set_vexpand: true,

                            #[watch]
                            set_description: {
                                if let Some(err) = model.game_error.as_deref() {
                                    Some(err)
                                } else if let Some(status) = model.loading.as_deref() {
                                    Some(status)
                                } else if !model.game_loaded {
                                    Some(crate::LOADING_GAME_INFO.as_str())
                                } else if model.state.is_none() {
                                    Some(crate::LOADING_LAUNCHER_STATE.as_str())
                                } else {
                                    None
                                }
                            },

                            #[watch]
                            set_visible: {
                                !model.game_loaded
                                    || model.state.is_none()
                                    || model.loading.is_some()
                            }
                        },

                        adw::PreferencesPage {
                            #[watch]
                            set_visible: {
                                model.game_loaded
                                    && model.state.is_some()
                                    && model.loading.is_none()
                            },

                            add = &adw::PreferencesGroup {
                                set_margin_top: 48,

                                #[watch]
                                set_visible: model.style == LauncherStyle::Modern,

                                gtk::Picture {
                                    set_resource: Some(&format!("{APP_RESOURCE_PATH}/icons/scalable/apps/{APP_ID}.png")),
                                    set_vexpand: true,
                                    set_content_fit: gtk::ContentFit::ScaleDown
                                },

                                gtk::Label {
                                    set_label: "Sleepy Launcher",
                                    set_margin_top: 32,
                                    add_css_class: "title-1"
                                }
                            },

                            add = &adw::PreferencesGroup {
                                #[watch]
                                set_valign: match model.style {
                                    LauncherStyle::Modern => gtk::Align::Center,
                                    LauncherStyle::Classic => gtk::Align::End
                                },

                                #[watch]
                                set_width_request: match model.style {
                                    LauncherStyle::Modern => -1,
                                    LauncherStyle::Classic => 800
                                },

                                #[watch]
                                set_visible: model.downloading,

                                set_vexpand: true,
                                set_margin_top: 48,
                                set_margin_bottom: 48,

                                add = model.progress_bar.widget(),
                            },

                            add = &adw::PreferencesGroup {
                                #[watch]
                                set_valign: match model.style {
                                    LauncherStyle::Modern => gtk::Align::Center,
                                    LauncherStyle::Classic => gtk::Align::End
                                },

                                #[watch]
                                set_width_request: match model.style {
                                    LauncherStyle::Modern => -1,
                                    LauncherStyle::Classic => 800
                                },

                                #[watch]
                                set_visible: !model.downloading,

                                #[watch]
                                set_margin_bottom: match model.style {
                                    LauncherStyle::Modern => 48,
                                    LauncherStyle::Classic => 0
                                },

                                set_vexpand: true,

                                gtk::Box {
                                    #[watch]
                                    set_halign: match model.style {
                                        LauncherStyle::Modern => gtk::Align::Center,
                                        LauncherStyle::Classic => gtk::Align::End
                                    },

                                    #[watch]
                                    set_height_request: match model.style {
                                        LauncherStyle::Modern => -1,
                                        LauncherStyle::Classic => 40
                                    },

                                    set_margin_top: 64,
                                    set_spacing: 8,

                                    // TODO: predownload UI is disabled until the SDK
                                    //       provides predownload support again.

                                    adw::Bin {
                                        set_css_classes: &["background", "round-bin"],

                                        #[watch]
                                        set_visible: !model.kill_game_button,

                                        gtk::Button {
                                            adw::ButtonContent {
                                                #[watch]
                                                set_icon_name: match &model.state {
                                                    Some(LauncherState::Launch) => "media-playback-start-symbolic",

                                                    Some(LauncherState::FolderMigrationRequired { .. }) |
                                                    Some(LauncherState::WineNotInstalled) |
                                                    Some(LauncherState::PrefixNotExists) |
                                                    Some(LauncherState::DxvkNotInstalled) |
                                                    Some(LauncherState::Dx12NotInstalled) => "document-save-symbolic",

                                                    Some(LauncherState::GameDiff(_)) => "document-save-symbolic",

                                                    None => "window-close-symbolic"
                                                },

                                                #[watch]
                                                set_label: &match &model.state {
                                                    Some(LauncherState::Launch) => i18n!("launch"),

                                                    Some(LauncherState::FolderMigrationRequired { .. }) => i18n!("migrate-folders"),

                                                    Some(LauncherState::WineNotInstalled) => i18n!("download-wine"),
                                                    Some(LauncherState::PrefixNotExists)  => i18n!("create-prefix"),
                                                    Some(LauncherState::DxvkNotInstalled) => i18n!("install-dxvk"),
                                                    Some(LauncherState::Dx12NotInstalled) => i18n!("install-dx12"),

                                                    Some(LauncherState::GameDiff(diff)) => match diff {
                                                        GameDiff::Download { .. } => i18n!("download"),
                                                        GameDiff::Update { .. }   => i18n!("update")
                                                    },

                                                    None => String::from("...")
                                                }
                                            },

                                            #[watch]
                                            set_sensitive: !model.disabled_buttons && model.state.is_some(),

                                            #[watch]
                                            set_css_classes: match &model.state {
                                                Some(_) => &["suggested-action", "pill"],
                                                None => &["pill"]
                                            },

                                            #[watch]
                                            set_tooltip_text: Some(&match &model.state {
                                                Some(LauncherState::FolderMigrationRequired { .. }) => i18n!("migrate-folders-tooltip"),

                                                _ => String::new()
                                            }),

                                            set_hexpand: false,
                                            set_width_request: 200,

                                            connect_clicked => AppMsg::PerformAction
                                        }
                                    },

                                    adw::Bin {
                                        set_css_classes: &["background", "round-bin"],

                                        #[watch]
                                        set_visible: model.kill_game_button,

                                        gtk::Button {
                                            adw::ButtonContent {
                                                set_icon_name: "violence-symbolic", // window-close-symbolic
                                                set_label: &i18n!("kill-game-process")
                                            },

                                            #[watch]
                                            set_sensitive: !model.disabled_kill_game_button,

                                            set_css_classes: &["error", "pill"],

                                            set_hexpand: false,
                                            set_width_request: 200,

                                            connect_clicked[sender] => move |_| {
                                                sender.input(AppMsg::DisableKillGameButton(true));

                                                std::thread::spawn(clone!(
                                                    #[strong]
                                                    sender,

                                                    move || {
                                                        std::thread::sleep(std::time::Duration::from_secs(3));

                                                        sender.input(AppMsg::DisableKillGameButton(false));
                                                    }
                                                ));

                                                let result = std::process::Command::new("pkill")
                                                    .arg("-f") // full text search
                                                    .arg("-i") // case-insensitive
                                                    .arg("ZenlessZoneZero")
                                                    .spawn();

                                                if let Err(err) = result {
                                                    sender.input(AppMsg::Toast {
                                                        title: i18n!("kill-game-process-failed"),
                                                        description: Some(err.to_string())
                                                    });
                                                }
                                            }
                                        }
                                    },

                                    adw::Bin {
                                        set_css_classes: &["background", "round-bin"],

                                        #[watch]
                                        set_visible: matches!(model.state.as_ref(),
                                            Some(LauncherState::GameDiff(GameDiff::Download { .. }))
                                        ) && !model.kill_game_button,

                                        #[name = "import_game_button"]
                                        gtk::Button {
                                            set_width_request: 44,
                                            set_css_classes: &["circular"],
                                            set_icon_name: "document-import-symbolic",
                                            set_tooltip_text: Some(&i18n!("import-game")),

                                            #[watch]
                                            set_sensitive: !model.disabled_buttons,

                                            connect_clicked[sender] => move |_| {
                                                sender.input(AppMsg::ImportGame);
                                            }
                                        }
                                    },

                                    adw::Bin {
                                        set_css_classes: &["background", "round-bin"],

                                        gtk::Button {
                                            #[watch]
                                            set_sensitive: !model.disabled_buttons,

                                            set_width_request: 44,

                                            add_css_class: "circular",
                                            set_icon_name: "emblem-system-symbolic",

                                            connect_clicked => AppMsg::OpenPreferences
                                        }
                                    }
                                }
                            }
                        }
                    },

                    set_measure_overlay: (&ui_contents, true),
                },
            },

            connect_close_request[sender] => move |_| {
                if let Err(err) = Config::flush() {
                    sender.input(AppMsg::Toast {
                        title: i18n!("config-update-error"),
                        description: Some(err.to_string())
                    });
                }

                gtk::glib::Propagation::Proceed
            }
        }
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>
    ) -> AsyncComponentParts<Self> {
        tracing::info!("Initializing main window");

        let model = App {
            progress_bar: ProgressBar::builder()
                .launch(ProgressBarInit {
                    caption: None,
                    display_progress: true,
                    display_fraction: true,
                    visible: true
                })
                .detach(),

            toast_overlay: adw::ToastOverlay::new(),

            loading: None,
            style: CONFIG.launcher.style,
            use_video_background: CONFIG.launcher.video_background,
            background_index: CONFIG.launcher.background_index,

            game_loaded: false,
            game_error: None,
            state: None,

            downloading: false,
            disabled_buttons: false,
            kill_game_button: false,
            disabled_kill_game_button: false
        };

        model.progress_bar.widget().set_halign(gtk::Align::Center);
        model.progress_bar.widget().set_width_request(360);

        let toast_overlay = &model.toast_overlay;

        let widgets = view_output!();

        // drag-and-drop onto the import button
        {
            let drop_target = gtk::DropTarget::new(
                gtk::gdk::FileList::static_type(),
                gtk::gdk::DragAction::COPY
            );

            drop_target.connect_drop(clone!(
                #[strong]
                sender,
                move |_, value, _, _| {
                    if let Ok(file_list) = value.get::<gtk::gdk::FileList>()
                        && let Some(file) = file_list.files().first()
                        && let Some(path) = file.path()
                    {
                        sender.input(AppMsg::ImportGameFromPath(path));

                        return true;
                    }

                    false
                }
            ));

            widgets.import_game_button.add_controller(drop_target);
        }

        unsafe {
            MAIN_WINDOW = Some(widgets.main_window.clone());

            PREFERENCES_WINDOW = Some(
                PreferencesApp::builder()
                    .launch(widgets.main_window.clone().into())
                    .forward(sender.input_sender(), std::convert::identity)
            );
        }

        let mut group = RelmActionGroup::<WindowActionGroup>::new();

        // TODO: reduce code somehow

        group.add_action::<LauncherFolder>(RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                if let Err(err) = open::that(LAUNCHER_FOLDER.as_path()) {
                    sender.input(AppMsg::Toast {
                        title: i18n!("launcher-folder-opening-error"),
                        description: Some(err.to_string())
                    });

                    tracing::error!("Failed to open launcher folder: {err}");
                }
            }
        )));

        group.add_action::<GameFolder>(RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                let path = match Config::get() {
                    Ok(config) => config.game.path
                        .for_edition(&config.launcher.edition)
                        .to_path_buf(),

                    Err(_) => CONFIG.game.path
                        .for_edition(&CONFIG.launcher.edition)
                        .to_path_buf()
                };

                if let Err(err) = open::that(path) {
                    sender.input(AppMsg::Toast {
                        title: i18n!("game-folder-opening-error"),
                        description: Some(err.to_string())
                    });

                    tracing::error!("Failed to open game folder: {err}");
                }
            }
        )));

        group.add_action::<ConfigFile>(RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                if let Ok(file) = config_file()
                    && let Err(err) = open::that(file)
                {
                    sender.input(AppMsg::Toast {
                        title: i18n!("config-file-opening-error"),
                        description: Some(err.to_string())
                    });

                    tracing::error!("Failed to open config file: {err}");
                }
            }
        )));

        group.add_action::<DebugFile>(RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                if let Err(err) = open::that(crate::DEBUG_FILE.as_os_str()) {
                    sender.input(AppMsg::Toast {
                        title: i18n!("debug-file-opening-error"),
                        description: Some(err.to_string())
                    });

                    tracing::error!("Failed to open debug file: {err}");
                }
            }
        )));

        group.add_action::<SignalSearchUrl>(RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                std::thread::spawn(clone!(
                    #[strong]
                    sender,
                    move || {
                        let config = Config::get().unwrap_or_else(|_| CONFIG.clone());

                        let web_cache = config.game.path.for_edition(&config.launcher.edition)
                            // TODO: the SDK no longer exposes a region-specific data
                            //       folder name. ZZZ uses the same folder in both regions.
                            .join("ZenlessZoneZero_Data")
                            .join("webCaches");

                        // Find newest cache folder
                        let mut web_cache_id = None;

                        if let Ok(entries) = web_cache.read_dir() {
                            for entry in entries.flatten() {
                                if entry.path().is_dir()
                                    && entry
                                        .file_name()
                                        .to_string_lossy()
                                        .trim_matches(|c| "0123456789.".contains(c))
                                        .is_empty()
                                    && Some(entry.file_name()) > web_cache_id
                                {
                                    web_cache_id = Some(entry.file_name());
                                }
                            }
                        }

                        if let Some(web_cache_id) = web_cache_id {
                            let web_cache =
                                web_cache.join(web_cache_id).join("Cache/Cache_Data/data_2");

                            match std::fs::read(web_cache) {
                                Ok(web_cache) => {
                                    let web_cache = String::from_utf8_lossy(&web_cache);

                                    // https://public-operation.[ho-yo-ver-se].com/common/gacha_record/getGachaLog?......
                                    if let Some(url) = web_cache
                                        .split("1/0/")
                                        .filter(|line| {
                                            line.starts_with("https://")
                                                && line.contains("getGachaLog")
                                        })
                                        .last()
                                        .map(|url| url.trim_end_matches('\0'))
                                    {
                                        if let Err(err) = open::that(url) {
                                            tracing::error!(
                                                "Failed to open Signal Search URL: {err}"
                                            );

                                            sender.input(AppMsg::Toast {
                                                title: i18n!("signal-search-url-opening-error"),
                                                description: Some(err.to_string())
                                            });
                                        }
                                    }

                                    else {
                                        tracing::error!("Couldn't find wishes URL: no url found");

                                        sender.input(AppMsg::Toast {
                                            title: i18n!("signal-search-url-search-failed"),
                                            description: None
                                        });
                                    }
                                }

                                Err(err) => {
                                    tracing::error!(
                                        "Couldn't find wishes URL: failed to open cache file: {err}"
                                    );

                                    sender.input(AppMsg::Toast {
                                        title: i18n!("signal-search-url-search-failed"),
                                        description: Some(err.to_string())
                                    });
                                }
                            }
                        }

                        else {
                            tracing::error!(
                                "Couldn't find Signal Search URL: cache file doesn't exist"
                            );

                            sender.input(AppMsg::Toast {
                                title: i18n!("signal-search-url-search-failed"),
                                description: None
                            });
                        }
                    }
                ));
            }
        )));

        group.add_action::<About>(RelmAction::new_stateless(move |_| unsafe {
            // I honestly don't care anymore.
            #[allow(static_mut_refs)]
            if let Some(window) = MAIN_WINDOW.as_ref() {
                AboutDialog::builder()
                    .launch(())
                    .detach()
                    .widget()
                    .present(Some(window));
            }
        }));

        widgets.main_window.insert_action_group("win", Some(&group.into_action_group()));

        tracing::info!("Main window initialized");

        // Load the game info in background so the window can be shown as soon
        // as possible. The status page will display the "loading game info"
        // description until `SetGameLoaded` message is received.
        relm4::spawn(clone!(
            #[strong]
            sender,

            async move {
                match crate::get_game_for_config().await {
                    Ok(_) => sender.input(AppMsg::SetGameLoaded(true)),

                    Err(err) => {
                        tracing::error!(?err, "failed to load game info");

                        sender.input(AppMsg::Toast {
                            // TODO: add a dedicated locale key for this error
                            title: i18n!("game-diff-finding-error"),
                            description: Some(err.to_string())
                        });

                        sender.input(AppMsg::SetGameLoaded(false));
                    }
                }
            }
        ));

        let config = Config::get().unwrap_or_else(|_| CONFIG.clone());

        // Download background picture/video if needed
        if config.launcher.style == LauncherStyle::Classic
            && !KEEP_BACKGROUND_FILE.exists()
        {
            let download_video = config.launcher.video_background;
            let background_index = config.launcher.background_index;

            relm4::spawn(clone!(
                #[strong]
                sender,

                async move {
                    if let Err(err) = crate::background::download_background(
                        download_video,
                        background_index
                    ).await {
                        tracing::error!("Failed to download background picture: {err}");

                        sender.input(AppMsg::Toast {
                            title: i18n!("background-downloading-failed"),
                            description: Some(err.to_string())
                        });
                    } else {
                        sender.input(AppMsg::ReloadCss);
                    }
                }
            ));
        }

        // Update components index
        {
            let sender = sender.clone();

            std::thread::spawn(move || {
                let components = ComponentsLoader::new(config.components.path.clone());

                match components.is_sync(&config.components.servers) {
                    Ok(Some(_)) => (),

                    Ok(None) => {
                        for host in &config.components.servers {
                            match components.sync(host) {
                                Ok(changes) => {
                                    sender.input(AppMsg::Toast {
                                        title: i18n!("components-index-updated"),
                                        description: if changes.is_empty() {
                                            None
                                        } else {
                                            Some(
                                                changes.into_iter()
                                                    .map(|line| format!("- {line}"))
                                                    .collect::<Vec<_>>()
                                                    .join("\n")
                                            )
                                        }
                                    });

                                    sender.input(AppMsg::ReloadComponents);

                                    break;
                                }

                                Err(err) => {
                                    tracing::error!("Failed to sync components index: {err}");

                                    sender.input(AppMsg::Toast {
                                        title: i18n!("components-index-sync-failed"),
                                        description: Some(err.to_string())
                                    });
                                }
                            }
                        }
                    }

                    Err(err) => {
                        tracing::error!("Failed to verify that components index synced: {err}");

                        sender.input(AppMsg::Toast {
                            title: i18n!("components-index-verify-failed"),
                            description: Some(err.to_string())
                        });
                    }
                }
            });
        }

        AsyncComponentParts {
            model,
            widgets
        }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>
    ) {
        tracing::debug!("Called main window event");

        match msg {
            // TODO: make function from this message like with toast
            AppMsg::UpdateLauncherState {
                perform_on_download_needed,
                show_status_page
            } => {
                if show_status_page {
                    self.loading = Some(i18n!("loading-launcher-state"));
                } else {
                    self.disabled_buttons = true;
                }

                relm4::spawn(clone!(
                    #[strong]
                    sender,

                    async move {
                        // Let the UI paint the status page before the state
                        // computation starts.
                        tokio::task::yield_now().await;

                        let game = match crate::get_game_for_config().await {
                            Ok(game) => game,

                            Err(err) => {
                                tracing::error!("Failed to load game info: {err}");

                                sender.input(AppMsg::Toast {
                                    title: i18n!("game-diff-finding-error"),
                                    description: Some(err.to_string())
                                });

                                sender.input(AppMsg::SetGameLoaded(false));
                                sender.input(AppMsg::SetLauncherState(None));
                                sender.input(AppMsg::SetLoadingStatus(Some(
                                    i18n!("launcher-state-updating-error")
                                )));

                                if !show_status_page {
                                    sender.input(AppMsg::DisableButtons(false));
                                }

                                return;
                            }
                        };

                        match LauncherState::get_from_config(game).await {
                            Ok(state) => {
                                let download_required = matches!(state, LauncherState::GameDiff(_));

                                sender.input(AppMsg::SetLauncherState(Some(state)));

                                if show_status_page {
                                    sender.input(AppMsg::SetLoadingStatus(None));
                                } else {
                                    sender.input(AppMsg::DisableButtons(false));
                                }

                                if perform_on_download_needed && download_required {
                                    sender.input(AppMsg::PerformAction);
                                }
                            }

                            Err(err) => {
                                tracing::error!("Failed to update launcher state: {err}");

                                sender.input(AppMsg::Toast {
                                    title: i18n!("launcher-state-updating-error"),
                                    description: Some(err.to_string())
                                });

                                sender.input(AppMsg::SetLauncherState(None));
                                sender.input(AppMsg::SetLoadingStatus(Some(
                                    i18n!("launcher-state-updating-error")
                                )));

                                if !show_status_page {
                                    sender.input(AppMsg::DisableButtons(false));
                                }
                            }
                        }
                    }
                ));
            }

            AppMsg::SetGameLoaded(loaded) => {
                if loaded {
                    self.game_loaded = true;
                    self.game_error = None;

                    sender.input(AppMsg::UpdateLauncherState {
                        perform_on_download_needed: false,
                        show_status_page: true
                    });
                } else {
                    self.game_error = Some(i18n!("game-diff-finding-error"));
                }
            }

            AppMsg::SetLauncherState(state) => {
                if state.is_some() {
                    self.game_loaded = true;
                    self.game_error = None;
                }

                self.state = state;

                // The app is now ready for the preferences to be writable.
                crate::READY.store(true, Ordering::Relaxed);
            }

            AppMsg::SetLoadingStatus(status) => {
                self.loading = status;
            }

            AppMsg::ReloadCss => {
                relm4::set_global_css(&crate::GLOBAL_CSS);
            }

            #[allow(unused_must_use)]
            AppMsg::UpdateGameVersion => unsafe {
                #[allow(static_mut_refs)]
                if let Some(preferences) = PREFERENCES_WINDOW.as_ref() {
                    preferences.sender().send(PreferencesAppMsg::UpdateGameVersion);
                }
            },

            #[allow(unused_must_use)]
            AppMsg::ReloadComponents => unsafe {
                #[allow(static_mut_refs)]
                if let Some(preferences) = PREFERENCES_WINDOW.as_ref() {
                    preferences.sender().send(PreferencesAppMsg::ReloadComponents);
                }
            },

            AppMsg::SetLauncherStyle(style) => {
                self.style = style;

                relm4::set_global_css(&crate::GLOBAL_CSS);
            }

            AppMsg::SetVideoBackground(use_video) => {
                self.use_video_background = use_video;

                relm4::set_global_css(&crate::GLOBAL_CSS);
            }

            AppMsg::SetBackgroundIndex(background_index) => {
                self.background_index = background_index
            }

            AppMsg::SetDownloading(state) => {
                self.downloading = state;
            }

            AppMsg::DisableButtons(state) => {
                self.disabled_buttons = state;
            }

            AppMsg::SetKillGameButton(state) => {
                self.kill_game_button = state;
            }

            AppMsg::DisableKillGameButton(state) => {
                self.disabled_kill_game_button = state;
            }

            #[allow(static_mut_refs)]
            AppMsg::OpenPreferences => {
                if let Some(preferences) = unsafe { PREFERENCES_WINDOW.as_ref() } {
                    preferences.widget().present();
                }
            },

            AppMsg::RepairGame => {
                let progress_bar_input = self.progress_bar.sender().clone();

                let Some(runtime) = crate::TOKIO_HANDLE.get().cloned() else {
                    tracing::error!("Tokio runtime handle is not initialized");
                    return;
                };

                std::thread::spawn(move || runtime.block_on(
                    repair_game::repair_game(sender, progress_bar_input)
                ));
            }

            AppMsg::RemakePrefix => {
                let config = match Config::get() {
                    Ok(config) => config,

                    Err(err) => {
                        sender.input(AppMsg::Toast {
                            title: i18n!("config-file-opening-error"),
                            description: Some(err.to_string())
                        });

                        return;
                    }
                };
                let prefix = config.game.wine.prefix.clone();

                if prefix.exists() && let Err(err) = std::fs::remove_dir_all(&prefix) {
                    tracing::error!("Failed to remove wine prefix: {err}");

                    sender.input(AppMsg::Toast {
                        title: i18n!("wine-prefix-update-failed"),
                        description: Some(err.to_string())
                    });

                    return;
                }

                sender.input(AppMsg::UpdateLauncherState {
                    perform_on_download_needed: false,
                    show_status_page: true
                });
            }

            AppMsg::ImportGame => {
                relm4::spawn(clone!(
                    #[strong]
                    sender,
                    async move {
                        if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                            sender.input(AppMsg::ImportGameFromPath(folder.path().to_path_buf()));
                        }
                    }
                ));
            }

            AppMsg::ImportGameFromPath(path) => {
                relm4::spawn(clone!(
                    #[strong]
                    sender,
                    async move { import_game::import_game(sender, path).await }
                ));
            }

            AppMsg::PerformAction => {
                let Some(state) = self.state.as_ref() else {
                    return;
                };

                match state {
                    LauncherState::Launch => launch::launch(sender),

                    LauncherState::WineNotInstalled => {
                        download_wine::download_wine(sender, self.progress_bar.sender().to_owned())
                    }

                    LauncherState::PrefixNotExists => create_prefix::create_prefix(sender),

                    LauncherState::DxvkNotInstalled => {
                        install_dxvk::install_dxvk(sender, self.progress_bar.sender().to_owned())
                    }

                    LauncherState::Dx12NotInstalled => {
                        install_dx12::install_dx12(sender, self.progress_bar.sender().to_owned())
                    }

                    LauncherState::FolderMigrationRequired {
                        from,
                        to,
                        cleanup_folder
                    } => migrate_folder::migrate_folder(
                        sender,
                        from.to_owned(),
                        to.to_owned(),
                        cleanup_folder.to_owned()
                    ),

                    // TODO: this should download voice packs as well once the
                    //       SDK exposes their components.
                    LauncherState::GameDiff(diff) => {
                        let diff = diff.clone();
                        let progress_bar_input = self.progress_bar.sender().clone();

                        let Some(runtime) = crate::TOKIO_HANDLE.get().cloned() else {
                            tracing::error!("Tokio runtime handle is not initialized");
                            return;
                        };

                        std::thread::spawn(move || runtime.block_on(
                            download_diff::download_diff(
                                sender,
                                progress_bar_input,
                                diff,
                                true
                            )
                        ));
                    }
                }
            }

            #[allow(static_mut_refs)]
            AppMsg::HideWindow => {
                if let Some(window) = unsafe { MAIN_WINDOW.as_ref() } {
                    window.set_visible(false);
                }
            },

            #[allow(static_mut_refs)]
            AppMsg::ShowWindow => {
                if let Some(window) = unsafe { MAIN_WINDOW.as_ref() } {
                    window.present();
                }
            },

            AppMsg::SuggestTimeoutFix => self.suggest_timeout_fix(),

            AppMsg::Toast {
                title,
                description
            } => self.toast(title, description)
        }
    }
}

impl App {
    pub fn suggest_timeout_fix(&self) {
        #[allow(static_mut_refs)]
        let Some(window) = (unsafe { MAIN_WINDOW.as_ref() })
        else {
            return;
        };

        let dialog = adw::MessageDialog::new(
            Some(window),
            Some(&i18n!("timeout-fix-detected")),
            Some(&i18n!("timeout-fix-detected-description"))
        );

        dialog.add_response("ignore", &i18n!("close"));
        dialog.add_response("enable", &i18n!("enable"));

        dialog.set_response_appearance("enable", adw::ResponseAppearance::Suggested);

        dialog.connect_response(Some("enable"), |_, _| {
            if let Ok(mut config) = Config::get() {
                config.game.wine.timeout_fix = true;

                Config::update(config);
            }

            #[allow(static_mut_refs)]
            unsafe {
                if let Some(prefs) = PREFERENCES_WINDOW.as_ref() {
                    prefs.emit(PreferencesAppMsg::SetTimeoutFix(true));
                }
            }
        });

        dialog.present();
    }

    pub fn toast<T: AsRef<str>>(&mut self, title: T, description: Option<T>) {
        let toast = adw::Toast::new(title.as_ref());

        toast.set_timeout(4);

        #[allow(static_mut_refs)]
        if let Some(description) = description {
            toast.set_button_label(Some(&i18n!("details")));

            #[allow(static_mut_refs)]
            let window = unsafe { MAIN_WINDOW.as_ref() };

            let dialog = adw::MessageDialog::new(
                window,
                Some(title.as_ref()),
                Some(description.as_ref())
            );

            dialog.add_response("close", &i18n!("close"));
            dialog.add_response("save", &i18n!("save"));

            dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);

            dialog.connect_response(Some("save"), |_, _| {
                if let Err(err) = open::that(crate::DEBUG_FILE.as_os_str()) {
                    tracing::error!("Failed to open debug file: {err}");
                }
            });

            toast.connect_button_clicked(move |_| {
                dialog.present();
            });
        }

        self.toast_overlay.add_toast(toast);
    }
}

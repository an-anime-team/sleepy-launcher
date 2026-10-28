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

use relm4::prelude::*;

use gtk::prelude::*;
use adw::prelude::*;

use anime_launcher_sdk::config::ConfigExt;
use anime_launcher_sdk::zzz::config::Config;
use anime_launcher_sdk::zzz::config::schema::launcher::LauncherStyle;

use crate::i18n;

use super::general::*;
use super::enhancements::*;

pub static mut PREFERENCES_WINDOW: Option<adw::PreferencesWindow> = None;

pub struct PreferencesApp {
    general: AsyncController<GeneralApp>,
    enhancements: AsyncController<EnhancementsApp>
}

#[derive(Debug, Clone)]
pub enum PreferencesAppMsg {
    SetLauncherStyle(LauncherStyle),
    UpdateGameVersion,
    ReloadComponents,
    SetVideoBackground(bool),
    SetBackgroundIndex(u8),

    UpdateLauncherState,
    RepairGame,
    RemakePrefix,
    SetTimeoutFix(bool),

    Toast {
        title: String,
        description: Option<String>
    }
}

#[relm4::component(async, pub)]
impl SimpleAsyncComponent for PreferencesApp {
    type Init = gtk::Window;
    type Input = PreferencesAppMsg;
    type Output = crate::ui::main::AppMsg;

    view! {
        preferences_window = adw::PreferencesWindow {
            set_title: Some(&i18n!("preferences")),
            set_default_size: (700, 560),

            set_hide_on_close: true,
            set_modal: true,
            set_search_enabled: true,

            add = model.general.widget(),
            add = model.enhancements.widget(),

            connect_close_request[sender] => move |_| {
                if let Err(err) = Config::flush() {
                    sender.input(PreferencesAppMsg::Toast {
                        title: i18n!("config-update-error"),
                        description: Some(err.to_string())
                    });
                }

                gtk::glib::Propagation::Proceed
            }
        }
    }

    async fn init(parent: Self::Init, root: Self::Root, sender: AsyncComponentSender<Self>) -> AsyncComponentParts<Self> {
        tracing::info!("Initializing preferences window");

        let model = Self {
            general: GeneralApp::builder()
                .launch(())
                .forward(sender.input_sender(), std::convert::identity),

            enhancements: EnhancementsApp::builder()
                .launch(())
                .forward(sender.input_sender(), std::convert::identity)
        };

        let widgets = view_output!();

        widgets.preferences_window.set_transient_for(Some(&parent));

        unsafe {
            PREFERENCES_WINDOW = Some(widgets.preferences_window.clone());
        }

        model.enhancements.emit(EnhancementsAppMsg::SetGamescopeParent);

        model.general.emit(GeneralAppMsg::UpdateDownloadedWine);
        model.general.emit(GeneralAppMsg::UpdateDownloadedDxvk);

        AsyncComponentParts { model, widgets }
    }

    async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
        tracing::debug!("Called preferences window event: {:?}", msg);

        match msg {
            #[allow(unused_must_use)]
            PreferencesAppMsg::UpdateGameVersion => {
                self.general.emit(GeneralAppMsg::UpdateGameVersion);
            }

            PreferencesAppMsg::ReloadComponents => {
                self.general.emit(GeneralAppMsg::ReloadComponents);
            }

            #[allow(unused_must_use)]
            PreferencesAppMsg::SetLauncherStyle(style) => {
                sender.output(Self::Output::SetLauncherStyle(style));
            }

            #[allow(unused_must_use)]
            PreferencesAppMsg::SetVideoBackground(use_video) => {
                sender.output(Self::Output::SetVideoBackground(use_video));
            }

            #[allow(unused_must_use)]
            PreferencesAppMsg::SetBackgroundIndex(background_index) => {
                sender.output(Self::Output::SetBackgroundIndex(background_index));
            }

            #[allow(unused_must_use)]
            PreferencesAppMsg::UpdateLauncherState => {
                sender.output(Self::Output::UpdateLauncherState {
                    perform_on_download_needed: false,
                    show_status_page: false
                });
            }

            #[allow(unused_must_use)]
            #[allow(static_mut_refs)]
            PreferencesAppMsg::RepairGame => {
                if let Some(window) = unsafe { PREFERENCES_WINDOW.as_ref() } {
                    window.close();
                }

                sender.output(Self::Output::RepairGame);
            }

            #[allow(unused_must_use)]
            #[allow(static_mut_refs)]
            PreferencesAppMsg::RemakePrefix => {
                if let Some(window) = unsafe { PREFERENCES_WINDOW.as_ref() } {
                    window.close();
                }

                sender.output(Self::Output::RemakePrefix);
            }

            PreferencesAppMsg::SetTimeoutFix(value) => {
                self.enhancements.emit(EnhancementsAppMsg::SetTimeoutFix(value));
            }

            PreferencesAppMsg::Toast { title, description } => unsafe {
                // I honestly don't care anymore.
                #[allow(static_mut_refs)]
                let toast = adw::Toast::new(&title);

                toast.set_timeout(4);

                #[allow(static_mut_refs)]
                if let Some(description) = description {
                    toast.set_button_label(Some(&i18n!("details")));

                    let dialog = adw::MessageDialog::new(PREFERENCES_WINDOW.as_ref(), Some(&title), Some(&description));

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

                #[allow(static_mut_refs)]
                if let Some(window) = PREFERENCES_WINDOW.as_ref() {
                    window.add_toast(toast);
                }
            }
        }
    }
}

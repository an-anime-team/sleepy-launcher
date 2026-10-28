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

use std::path::PathBuf;

use gtk::prelude::*;
use adw::prelude::*;

use anime_launcher_sdk::config::ConfigExt;
use anime_launcher_sdk::download::{Installer, Update as InstallerUpdate};
use anime_launcher_sdk::zzz::config::Config;

use crate::*;

use super::ComponentGroupMsg;
use super::progress_bar::ProgressBarMsg;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionState {
    Downloaded,
    Downloading,
    NotDownloaded
}

pub struct ComponentVersion {
    pub name: String,
    pub title: String,
    pub recommended: bool,

    pub download_uri: String,
    pub download_folder: PathBuf,
    pub download_filename: Option<String>,

    pub show_recommended_only: bool,
    pub state: VersionState,

    pub progress_bar: AsyncController<super::ProgressBar>
}

#[derive(Debug)]
pub enum ComponentVersionMsg {
    ShowRecommendedOnly(bool),
    PerformAction,
    SetState(VersionState)
}

#[relm4::component(async, pub)]
impl SimpleAsyncComponent for ComponentVersion {
    type Init = (super::ComponentsListVersion, PathBuf);
    type Input = ComponentVersionMsg;
    type Output = ComponentGroupMsg;

    view! {
        row = adw::ActionRow {
            set_title: &model.title,

            #[watch]
            set_visible: !model.show_recommended_only || model.recommended || model.state != VersionState::NotDownloaded,

            add_suffix = &gtk::Button {
                #[watch]
                set_icon_name: match model.state {
                    VersionState::NotDownloaded => "document-save-symbolic",

                    // In other states it will be downloaded or hidden
                    _ => "user-trash-symbolic"
                },

                add_css_class: "flat",
                set_valign: gtk::Align::Center,

                #[watch]
                set_visible: model.state != VersionState::Downloading,

                connect_clicked => ComponentVersionMsg::PerformAction
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let mut model = ComponentVersion {
            name: init.0.name.clone(),
            title: init.0.title,
            recommended: init.0.recommended,

            download_uri: init.0.uri,
            download_folder: init.1,
            download_filename: init.0.format.map(|format| format!("{}.{format}", init.0.name)),

            show_recommended_only: true,
            state: VersionState::NotDownloaded,

            progress_bar: super::ProgressBar::builder()
                .launch(super::ProgressBarInit {
                    caption: None,
                    display_progress: true,
                    display_fraction: false,
                    visible: false,
                })
                .detach()
        };

        // Set default component state
        model.state = if model.download_folder.join(&model.name).exists() {
            VersionState::Downloaded
        } else {
            VersionState::NotDownloaded
        };

        // Set progress bar width
        model.progress_bar.widget().set_width_request(200);

        let widgets = view_output!();

        widgets.row.add_suffix(model.progress_bar.widget());

        AsyncComponentParts { model, widgets }
    }

    async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
        match msg {
            ComponentVersionMsg::ShowRecommendedOnly(state) => self.show_recommended_only = state,

            ComponentVersionMsg::PerformAction => {
                match self.state {
                    VersionState::Downloaded => {
                        let path = self.download_folder.join(&self.name);

                        if path.exists() {
                            // To hide main button while it's deleting component's folder
                            self.state = VersionState::Downloading;

                            if let Err(err) = std::fs::remove_dir_all(&path) {
                                tracing::error!(
                                    "Failed to delete component '{}': {err}",
                                    self.name
                                );

                                self.state = VersionState::Downloaded;

                                return;
                            }
                        }

                        self.state = VersionState::NotDownloaded;

                        #[allow(unused_must_use)] {
                            sender.output(ComponentGroupMsg::CallOnDeleted);
                        }
                    }

                    VersionState::NotDownloaded => {
                        let config = Config::get().unwrap_or_else(|_| CONFIG.clone());

                        let installer = match Installer::new(&self.download_uri) {
                            Ok(installer) => installer.with_temp_folder(
                                config.launcher.temp.clone()
                                    .unwrap_or_else(std::env::temp_dir)
                            ),

                            Err(err) => {
                                tracing::error!(
                                    "Failed to create installer for component '{}': {err}",
                                    self.name
                                );

                                return;
                            }
                        };

                        let installer = match &self.download_filename {
                            Some(filename) => installer.with_filename(filename),
                            None => installer
                        };

                        self.state = VersionState::Downloading;

                        let download_folder = self.download_folder.clone();
                        let progress_bar_sender = self.progress_bar.sender().clone();
                        let sender = sender.clone();

                        relm4::spawn(async move {
                            let result = installer.install(download_folder, {
                                let progress_bar_sender = progress_bar_sender.clone();

                                move |state| {
                                    update_progress_bar(&progress_bar_sender, &state);
                                }
                            }).await;

                            let _ = progress_bar_sender.send(ProgressBarMsg::SetVisible(false));

                            match result {
                                Ok(()) => {
                                    sender.input(ComponentVersionMsg::SetState(
                                        VersionState::Downloaded
                                    ));

                                    let _ = sender.output(ComponentGroupMsg::CallOnDownloaded);
                                }

                                Err(err) => {
                                    tracing::error!("Failed to download component: {err}");

                                    sender.input(ComponentVersionMsg::SetState(
                                        VersionState::NotDownloaded
                                    ));
                                }
                            }
                        });
                    }

                    _ => ()
                }
            }

            ComponentVersionMsg::SetState(state) => self.state = state
        }
    }

}

fn update_progress_bar(progress_bar_input: &relm4::Sender<ProgressBarMsg>, state: &InstallerUpdate) {
    match state {
        InstallerUpdate::DownloadingStarted(_) => {
            let _ = progress_bar_input.send(ProgressBarMsg::Reset);
            let _ = progress_bar_input.send(ProgressBarMsg::SetVisible(true));
            let _ = progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(
                i18n!("downloading")
            )));
        }

        InstallerUpdate::DownloadingProgress(current, total) => {
            let _ = progress_bar_input.send(ProgressBarMsg::UpdateProgress(*current, *total));
        }

        InstallerUpdate::DownloadingFinished => (),

        InstallerUpdate::DownloadingError(err) => {
            tracing::error!("Component downloading failed: {err}");
        }

        InstallerUpdate::UnpackingStarted(_) => {
            let _ = progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(
                i18n!("unpacking")
            )));
        }

        InstallerUpdate::UnpackingFinished => {
            let _ = progress_bar_input.send(ProgressBarMsg::SetVisible(false));
        }

        InstallerUpdate::UnpackingError(err) => {
            tracing::error!("Component unpacking failed: {err}");
        }
    }
}

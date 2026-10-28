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

use relm4::Sender;
use gtk::glib::clone;

use anime_launcher_sdk::components::wine;
use anime_launcher_sdk::download::{Installer, Update as InstallerUpdate};

use crate::*;
use crate::ui::components::*;

use super::{App, AppMsg};

fn update_progress_bar(
    progress_bar_input: &Sender<ProgressBarMsg>,
    state: &InstallerUpdate
) {
    match state {
        InstallerUpdate::DownloadingStarted(_) => {
            let _ = progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(
                i18n!("downloading")
            )));
        }

        InstallerUpdate::DownloadingProgress(current, total) => {
            let _ = progress_bar_input.send(ProgressBarMsg::UpdateProgress(*current, *total));
        }

        InstallerUpdate::DownloadingFinished => (),

        InstallerUpdate::DownloadingError(err) => {
            tracing::error!("Wine downloading failed: {err}");
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
            tracing::error!("Wine unpacking failed: {err}");
        }
    }
}

pub fn download_wine(
    sender: AsyncComponentSender<App>,
    progress_bar_input: Sender<ProgressBarMsg>
) {
    let mut config = match Config::get() {
        Ok(config) => config,

        Err(err) => {
            sender.input(AppMsg::Toast {
                title: i18n!("config-file-opening-error"),
                description: Some(err.to_string())
            });

            return;
        }
    };

    match wine::get_downloaded(&CONFIG.components.path, &config.game.wine.builds) {
        Ok(downloaded) => {
            // Select downloaded version
            if !downloaded.is_empty() {
                config.game.wine.selected = Some(downloaded[0].versions[0].name.clone());

                Config::update(config);

                sender.input(AppMsg::UpdateLauncherState {
                    perform_on_download_needed: false,
                    show_status_page: true
                });
            }

            // Or download new one if none is available
            else {
                let latest = match wine::Version::latest(&CONFIG.components.path) {
                    Ok(version) => version,

                    Err(err) => {
                        tracing::error!("Failed to get latest wine version: {err}");

                        sender.input(AppMsg::Toast {
                            title: i18n!("wine-install-failed"),
                            description: Some(err.to_string())
                        });

                        return;
                    }
                };

                // Choose selected wine version or use latest available one
                let wine = match &config.game.wine.selected {
                    Some(version) => match wine::Version::find_in(&config.components.path, version) {
                        Ok(Some(version)) => version,
                        _ => latest
                    }

                    None => latest
                };

                let installer = match Installer::new(wine.uri) {
                    Ok(installer) => installer.with_temp_folder(
                        config.launcher.temp.clone()
                            .unwrap_or_else(std::env::temp_dir)
                    ),

                    Err(err) => {
                        sender.input(AppMsg::Toast {
                            title: i18n!("wine-install-failed"),
                            description: Some(err.to_string())
                        });

                        return;
                    }
                };

                sender.input(AppMsg::SetDownloading(true));

                std::thread::spawn(clone!(
                    #[strong]
                    sender,

                    move || {
                        let Some(runtime) = crate::TOKIO_HANDLE.get().cloned() else {
                            tracing::error!("Tokio runtime handle is not initialized");

                            sender.input(AppMsg::Toast {
                                title: i18n!("wine-install-failed"),
                                description: None
                            });

                            return;
                        };

                        let _ = progress_bar_input.send(ProgressBarMsg::Reset);
                        let _ = progress_bar_input.send(ProgressBarMsg::SetVisible(true));

                        let result = runtime.block_on(installer.install(&config.game.wine.builds, {
                            let sender = sender.clone();
                            let progress_bar_input = progress_bar_input.clone();

                            move |state| {
                                match &state {
                                    InstallerUpdate::DownloadingError(err) => {
                                        tracing::error!("Downloading failed: {err}");

                                        sender.input(AppMsg::Toast {
                                            title: i18n!("downloading-failed"),
                                            description: Some(err.clone())
                                        });
                                    }

                                    InstallerUpdate::UnpackingError(err) => {
                                        tracing::error!("Unpacking failed: {err}");

                                        sender.input(AppMsg::Toast {
                                            title: i18n!("unpacking-failed"),
                                            description: Some(err.clone())
                                        });
                                    }

                                    _ => ()
                                }

                                update_progress_bar(&progress_bar_input, &state);
                            }
                        }));

                        let _ = progress_bar_input.send(ProgressBarMsg::SetVisible(false));
                        sender.input(AppMsg::SetDownloading(false));

                        if let Err(err) = result {
                            tracing::error!("Failed to download wine: {err}");

                            sender.input(AppMsg::Toast {
                                title: i18n!("wine-install-failed"),
                                description: Some(err.to_string())
                            });

                            return;
                        }

                        config.game.wine.selected = Some(wine.name.clone());

                        Config::update(config);

                        sender.input(AppMsg::UpdateLauncherState {
                            perform_on_download_needed: false,
                            show_status_page: true
                        });
                    }
                ));
            }
        }

        Err(err) => sender.input(AppMsg::Toast {
            title: i18n!("downloaded-wine-list-failed"),
            description: Some(err.to_string())
        })
    }
}

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

use anime_launcher_sdk::wincompatlib::prelude::*;

use anime_launcher_sdk::config::ConfigExt;
use anime_launcher_sdk::zzz::config::Config;

use anime_launcher_sdk::components::dxvk::Version;
use anime_launcher_sdk::download::{Installer, Update as InstallerUpdate};

use crate::*;
use crate::ui::components::*;

use super::{App, AppMsg};

fn update_progress_bar(progress_bar_input: &Sender<ProgressBarMsg>, state: &InstallerUpdate) {
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
            tracing::error!("DXVK downloading failed: {err}");
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
            tracing::error!("DXVK unpacking failed: {err}");
        }
    }
}

pub fn install_dxvk(
    sender: AsyncComponentSender<App>,
    progress_bar_input: Sender<ProgressBarMsg>
) {
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

    match config.get_selected_wine() {
        Ok(Some(wine_config)) => {
            sender.input(AppMsg::DisableButtons(true));

            std::thread::spawn(clone!(
                #[strong] sender,
                move || {
                    let Some(runtime) = crate::TOKIO_HANDLE.get().cloned() else {
                        tracing::error!("Tokio runtime handle is not initialized");

                        sender.input(AppMsg::Toast {
                            title: i18n!("dxvk-install-failed"),
                            description: None
                        });

                        return;
                    };

                    let components_path = config.components.path.clone();

                    let wine = wine_config
                        .to_wine(
                            components_path.clone(),
                            Some(config.game.wine.builds.join(&wine_config.name)),
                        )
                        .with_prefix(config.game.wine.prefix)
                        .with_loader(WineLoader::Current);

                    let latest = match Version::latest(components_path) {
                        Ok(version) => version,
                        Err(err) => {
                            tracing::error!("Failed to get latest DXVK version: {}", err);
                            sender.input(AppMsg::Toast {
                                title: i18n!("dxvk-install-failed"),
                                description: Some(err.to_string()),
                            });
                            sender.input(AppMsg::DisableButtons(false));
                            return;
                        }
                    };

                    let dxvk_folder = config.game.dxvk.builds.join(&latest.name);

                    if !dxvk_folder.exists() {
                        let installer = match Installer::new(latest.uri) {
                            Ok(installer) => installer.with_temp_folder(
                                config.launcher.temp.clone()
                                    .unwrap_or_else(std::env::temp_dir)
                            ),
                            Err(err) => {
                                tracing::error!("Failed to create DXVK installer: {err}");

                                sender.input(AppMsg::Toast {
                                    title: i18n!("dxvk-download-error"),
                                    description: Some(err.to_string())
                                });

                                sender.input(AppMsg::DisableButtons(false));

                                return;
                            }
                        };

                        sender.input(AppMsg::SetDownloading(true));

                        let _ = progress_bar_input.send(ProgressBarMsg::Reset);
                        let _ = progress_bar_input.send(ProgressBarMsg::SetVisible(true));

                        let result = runtime.block_on(installer.install(&config.game.dxvk.builds, {
                            let sender = sender.clone();
                            let progress_bar_input = progress_bar_input.clone();

                            move |state| {
                                match &state {
                                    InstallerUpdate::DownloadingError(err) => {
                                        sender.input(AppMsg::Toast {
                                            title: i18n!("dxvk-download-error"),
                                            description: Some(err.clone())
                                        });
                                    }

                                    InstallerUpdate::UnpackingError(err) => {
                                        sender.input(AppMsg::Toast {
                                            title: i18n!("dxvk-unpack-error"),
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
                            tracing::error!("Failed to download DXVK: {err}");

                            sender.input(AppMsg::Toast {
                                title: i18n!("dxvk-download-error"),
                                description: Some(err.to_string())
                            });

                            sender.input(AppMsg::DisableButtons(false));
                            sender.input(AppMsg::UpdateLauncherState {
                                perform_on_download_needed: false,
                                show_status_page: true
                            });

                            return;
                        }
                    }

                    if let Err(err) = Dxvk::install(&wine, dxvk_folder, InstallParams::default()) {
                        tracing::error!("Failed to install DXVK: {}", err);
                        sender.input(AppMsg::Toast {
                            title: i18n!("dxvk-install-failed"),
                            description: Some(err.to_string()),
                        });
                    }

                    sender.input(AppMsg::DisableButtons(false));
                    sender.input(AppMsg::UpdateLauncherState {
                        perform_on_download_needed: false,
                        show_status_page: true,
                    });
                }
            ));
        }

        Ok(None) => {
            tracing::error!("Failed to get selected wine executable");
            sender.input(AppMsg::Toast {
                title: i18n!("failed-get-selected-wine"),
                description: None,
            });
        }

        Err(err) => {
            tracing::error!("Failed to get selected wine executable: {err}");
            sender.input(AppMsg::Toast {
                title: i18n!("failed-get-selected-wine"),
                description: Some(err.to_string()),
            });
        }
    }
}

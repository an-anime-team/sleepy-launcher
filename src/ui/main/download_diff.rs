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

use std::sync::Arc;

use relm4::Sender;

use anime_launcher_sdk::anime_game_core::game::{GameDiff, GameDiffOptions};
use anime_launcher_sdk::anime_game_core::sophon_tools::export::tokio::runtime::Handle;
use anime_launcher_sdk::anime_game_core::sophon_tools::updater::SophonUpdaterProgressMsg;
use anime_launcher_sdk::zzz::consts::VOICEOVER_COMPONENTS;

use crate::*;
use crate::ui::components::*;

use super::{App, AppMsg};

#[allow(unused_must_use)]
pub async fn download_diff(
    sender: AsyncComponentSender<App>,
    progress_bar_input: Sender<ProgressBarMsg>,
    diff: GameDiff<'static>,
    include_voiceovers: bool
) {
    sender.input(AppMsg::SetDownloading(true));
    sender.input(AppMsg::DisableButtons(true));

    progress_bar_input.send(ProgressBarMsg::Reset);
    progress_bar_input.send(ProgressBarMsg::SetVisible(true));
    progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(i18n!("downloading"))));

    let progress = {
        let progress_bar_input = progress_bar_input.clone();

        Arc::new(move |message: SophonUpdaterProgressMsg| {
            match message {
                SophonUpdaterProgressMsg::Verify { current, total, .. } => {
                    progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(
                        i18n!("verifying-files")
                    )));

                    progress_bar_input.send(ProgressBarMsg::UpdateProgress(current, total));
                }

                SophonUpdaterProgressMsg::Download { current, total } => {
                    progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(
                        i18n!("downloading")
                    )));

                    progress_bar_input.send(ProgressBarMsg::UpdateProgress(current, total));
                }

                SophonUpdaterProgressMsg::Patch { current, total, .. } => {
                    progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(
                        i18n!("applying-hdiff")
                    )));

                    progress_bar_input.send(ProgressBarMsg::UpdateProgress(current, total));
                }
            }
        })
    };

    let config = Config::get().unwrap_or_else(|_| CONFIG.clone());
    let downloader = config.launcher.downloader;

    let target_memory_usage = if downloader.target_memory_usage > 0 {
        Some(downloader.target_memory_usage * 1024 * 1024)
    } else {
        None
    };

    // Sophon treats 0 as unlimited, so always pass the value through.
    let max_parallel_streams = Some(downloader.max_parallel_streams);

    let base_options = GameDiffOptions {
        runtime: Some(Handle::current()),
        component: None,
        target_memory_usage,
        max_parallel_streams,
        chunks_dir: None,
        fast_verify: downloader.fast_verify,
        progress
    };

    let game_result = diff.apply(base_options.clone()).await;

    if let Err(err) = &game_result {
        tracing::error!("Downloading failed: {err}");

        sender.input(AppMsg::Toast {
            title: i18n!("downloading-failed"),
            description: Some(err.to_string())
        });
    }

    // Download/update voiceovers as well.
    if include_voiceovers && game_result.is_ok() {
        for voiceover in &config.game.voices {
            if !VOICEOVER_COMPONENTS.contains(&voiceover.as_str()) {
                tracing::warn!("Skipping unknown voiceover component: {voiceover}");

                continue;
            }

            progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(
                format!("{} ({voiceover})", i18n!("downloading"))
            )));

            let mut options = base_options.clone();

            options.component = Some(voiceover.clone());

            if let Err(err) = diff.apply(options).await {
                tracing::error!("Failed to download voiceover '{voiceover}': {err}");

                sender.input(AppMsg::Toast {
                    title: i18n!("downloading-failed"),
                    description: Some(format!("{voiceover}: {err}"))
                });
            }
        }
    }

    if game_result.is_ok() {
        sender.input(AppMsg::UpdateGameVersion);
    }

    progress_bar_input.send(ProgressBarMsg::SetVisible(false));
    sender.input(AppMsg::SetDownloading(false));
    sender.input(AppMsg::DisableButtons(false));

    sender.input(AppMsg::UpdateLauncherState {
        perform_on_download_needed: false,
        show_status_page: false
    });
}

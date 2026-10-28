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

use crate::*;
use crate::ui::components::*;

use super::{App, AppMsg};
use super::download_diff;

pub async fn repair_game(
    sender: AsyncComponentSender<App>,
    progress_bar_input: Sender<ProgressBarMsg>
) {
    let game = match crate::get_game_for_config().await {
        Ok(game) => game,

        Err(err) => {
            tracing::error!("Failed to repair game: {err}");

            sender.input(AppMsg::Toast {
                title: i18n!("game-file-repairing-error"),
                description: Some(err.to_string())
            });

            return;
        }
    };

    match game.repair().await {
        Ok(diff) => download_diff::download_diff(sender, progress_bar_input, diff, false).await,

        Err(err) => {
            tracing::error!("Failed to get game repair diff: {err}");

            sender.input(AppMsg::Toast {
                title: i18n!("game-file-repairing-error"),
                description: Some(err.to_string())
            });
        }
    }
}

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

use crate::*;

use super::{App, AppMsg};

pub fn migrate_folder(
    sender: AsyncComponentSender<App>,
    from: PathBuf,
    to: PathBuf,
    cleanup_folder: Option<PathBuf>
) {
    sender.input(AppMsg::DisableButtons(true));

    std::thread::spawn(move || {
        if let Err(err) = move_files::move_files(&from, &to) {
            tracing::error!("Failed to perform folders migration: {err}");

            sender.input(AppMsg::Toast {
                title: i18n!("folders-migration-error"),
                description: Some(err.to_string())
            });

            sender.input(AppMsg::DisableButtons(false));

            return;
        }

        if let Some(cleanup_folder) = cleanup_folder
            && let Err(err) = std::fs::remove_dir_all(cleanup_folder)
        {
            tracing::error!("Failed to remove cleanup folder: {err}");

            sender.input(AppMsg::Toast {
                title: i18n!("folders-migration-error"),
                description: Some(err.to_string())
            });
        }

        sender.input(AppMsg::DisableButtons(false));
        sender.input(AppMsg::UpdateLauncherState {
            perform_on_download_needed: false,
            show_status_page: true
        });
    });
}

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

use anime_launcher_sdk::anime_game_core::sophon_tools::region::SophonRegion;

use crate::*;
use super::{App, AppMsg};

pub async fn import_game(sender: relm4::AsyncComponentSender<App>, path: PathBuf) {
    // reject xdg-document-portal fuse mounts, files appear present but vanish
    // when the portal closes, so the game would silently break
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    for p in [path.as_path(), canonical.as_path()] {
        let s = p.to_string_lossy();
        if s.starts_with("/run/user/") && s.contains("/doc/") {
            sender.input(AppMsg::Toast {
                title: i18n!("import-game-path-runtime"),
                description: None
            });
            return;
        }
    }

    let mut config = match Config::get() {
        Ok(c) => c,
        Err(err) => {
            sender.input(AppMsg::Toast {
                title: i18n!("import-game-error"),
                description: Some(err.to_string())
            });
            return;
        }
    };

    // TODO: this duplicates the binary name from the SDK/core. There's no
    //       public API to check whether the game is installed yet.
    if !path.join("ZenlessZoneZero.exe").exists() {
        sender.input(AppMsg::Toast {
            title: i18n!("import-game-invalid-path"),
            description: None
        });
        return;
    }

    match config.launcher.edition {
        SophonRegion::Global => config.game.path.global = path,
        SophonRegion::China => config.game.path.china = path
    }

    Config::update(config);

    sender.input(AppMsg::UpdateGameVersion);

    sender.input(AppMsg::UpdateLauncherState {
        perform_on_download_needed: false,
        show_status_page: true
    });
}

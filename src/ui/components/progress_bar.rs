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

use adw::prelude::*;

use crate::*;

/// Format bytes into a human-readable string.
///
/// TODO: this used to be provided by `anime_game_core::prelude`.
#[inline]
pub fn prettify_bytes(bytes: u64) -> String {
    if bytes > 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    }

    else if bytes > 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / 1024.0 / 1024.0)
    }

    else if bytes > 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    }

    else {
        format!("{:.2} B", bytes)
    }
}

pub struct ProgressBarInit {
    pub caption: Option<String>,

    /// Add progress percentage (`XX.YY%`) suffix
    pub display_progress: bool,

    /// Add `(XX MB of YY MB)` suffix
    pub display_fraction: bool,

    pub visible: bool
}

pub struct ProgressBar {
    pub fraction: f64,
    pub caption: Option<String>,

    /// e.g. (53.21 MB, 10 GB)
    pub downloaded: Option<(String, String)>,

    /// Add progress percentage (`XX.YY%`) suffix
    pub display_progress: bool,

    /// Add `(XX MB of YY MB)` suffix
    pub display_fraction: bool,

    pub visible: bool
}

#[derive(Debug)]
pub enum ProgressBarMsg {
    Reset,
    UpdateCaption(Option<String>),
    DisplayProgress(bool),
    DisplayFraction(bool),

    /// (current bytes, total bytes) 
    UpdateProgress(u64, u64),

    // TODO: re-enable once the SDK provides installer/diff progress updates.
    // UpdateFromState(DiffUpdate),
    SetVisible(bool)
}

#[relm4::component(pub, async)]
impl SimpleAsyncComponent for ProgressBar {
    type Init = ProgressBarInit;
    type Input = ProgressBarMsg;
    type Output = ();

    view! {
        #[root]
        gtk::ProgressBar {
            set_valign: gtk::Align::Center,

            #[watch]
            set_visible: model.visible,

            #[watch]
            set_fraction: model.fraction,

            #[watch]
            set_show_text: model.caption.is_some(),

            #[watch]
            set_text: Some(&match model.caption.clone() {
                Some(mut caption) => {
                    if model.display_progress {
                        caption = format!("{caption}: {:.2}%", model.fraction * 100.0);
                    }

                    if model.display_fraction && let Some((curr, total)) = &model.downloaded {
                        caption = format!("{caption} ({curr} of {total})");
                    }

                    caption
                },
                None => String::new()
            })
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = ProgressBar {
            fraction: 0.0,
            caption: init.caption,
            downloaded: None,
            display_progress: init.display_progress,
            display_fraction: init.display_fraction,
            visible: init.visible
        };

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(&mut self, msg: Self::Input, _sender: AsyncComponentSender<Self>) {
        match msg {
            ProgressBarMsg::Reset => {
                self.fraction = 0.0;
                self.downloaded = None;
                self.caption = None;
            }

            ProgressBarMsg::UpdateCaption(caption) => self.caption = caption,
            ProgressBarMsg::DisplayProgress(value) => self.display_progress = value,
            ProgressBarMsg::DisplayFraction(value) => self.display_fraction = value,

            ProgressBarMsg::UpdateProgress(curr, total) => {
                if total > 0 {
                    self.fraction = curr as f64 / total as f64;
                }

                self.downloaded = Some((
                    prettify_bytes(curr),
                    prettify_bytes(total)
                ));
            }

            // TODO: re-enable once the SDK provides installer/diff progress updates.
            // ProgressBarMsg::UpdateFromState(state) => {
            //     match state {
            //         DiffUpdate::CheckingFreeSpace(_) |
            //         DiffUpdate::InstallerUpdate(InstallerUpdate::CheckingFreeSpace(_)) => self.caption = Some(i18n!("checking-free-space")),
            //
            //         DiffUpdate::InstallerUpdate(InstallerUpdate::DownloadingStarted(_))         => self.caption = Some(i18n!("downloading")),
            //         DiffUpdate::InstallerUpdate(InstallerUpdate::UpdatingPermissionsStarted(_)) => self.caption = Some(i18n!("updating-permissions")),
            //         DiffUpdate::InstallerUpdate(InstallerUpdate::UnpackingStarted(_))           => self.caption = Some(i18n!("unpacking")),
            //
            //         ...
            //     }
            // }

            ProgressBarMsg::SetVisible(visible) => self.visible = visible
        }
    }
}

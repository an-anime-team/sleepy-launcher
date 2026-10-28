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

use std::path::{Path, PathBuf};

use relm4::Sender;

use gtk::glib::clone;

use anime_launcher_sdk::wincompatlib::prelude::*;

use anime_launcher_sdk::config::ConfigExt;
use anime_launcher_sdk::zzz::config::Config;

use anime_launcher_sdk::anime_game_core::sophon_tools::export::reqwest;
use anime_launcher_sdk::download::{Installer, Update as InstallerUpdate};

use crate::*;
use crate::ui::components::*;

use super::{App, AppMsg};

const VKD3D_REPO: &str = "HansKristian-Work/vkd3d-proton";
const NVAPI_REPO: &str = "jp7677/dxvk-nvapi";

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
            tracing::error!("DX12 downloading failed: {err}");
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
            tracing::error!("DX12 unpacking failed: {err}");
        }
    }
}

/// Find the download URL of the latest release asset whose name ends with `suffix`.
async fn latest_release_asset(repo: &str, suffix: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("sleepy-launcher")
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client
        .get(format!("https://api.github.com/repos/{repo}/releases/latest"))
        .send()
        .await?
        .error_for_status()?;

    let json: serde_json::Value = serde_json::from_str(&response.text().await?)?;

    let assets = json.get("assets")
        .and_then(|value| value.as_array())
        .ok_or_else(|| anyhow::anyhow!("github release response for {repo} has no assets"))?;

    for asset in assets {
        let name = asset.get("name").and_then(|value| value.as_str());
        let url = asset.get("browser_download_url").and_then(|value| value.as_str());

        if let Some(name) = name
            && let Some(url) = url
            && name.ends_with(suffix)
        {
            return Ok(url.to_string());
        }
    }

    anyhow::bail!("no {suffix} asset found in the latest {repo} release")
}

/// Download and unpack the archive into a temporary folder.
async fn unpack_release(
    uri: &str,
    temp: &Path,
    extract_dir: &Path,
    progress_bar_input: &Sender<ProgressBarMsg>
) -> anyhow::Result<()> {
    let installer = Installer::new(uri)?.with_temp_folder(temp.to_path_buf());
    let archive_path = temp.join(installer.get_filename());

    let _ = std::fs::remove_dir_all(extract_dir);

    let _ = progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(
        i18n!("downloading")
    )));
    let _ = progress_bar_input.send(ProgressBarMsg::SetVisible(true));

    let result = installer.install(extract_dir, {
        let progress_bar_input = progress_bar_input.clone();

        move |state| update_progress_bar(&progress_bar_input, &state)
    }).await;

    let _ = progress_bar_input.send(ProgressBarMsg::SetVisible(false));
    let _ = std::fs::remove_file(archive_path);

    result
}

// find the dir that directly holds the x64 folder to make up for
// whether the archive wraps its contents in a version folder
fn find_arch_root(root: &Path) -> anyhow::Result<PathBuf> {
    if root.join("x64").is_dir() {
        return Ok(root.to_path_buf());
    }

    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();

        if path.is_dir() && path.join("x64").is_dir() {
            return Ok(path);
        }
    }

    anyhow::bail!("could not find an x64 folder in the extracted archive")
}

fn copy_dll(from: &Path, to_dir: &Path) -> anyhow::Result<()> {
    let name = from.file_name()
        .ok_or_else(|| anyhow::anyhow!("invalid dll path: {}", from.display()))?;

    std::fs::copy(from, to_dir.join(name))?;

    Ok(())
}

// locate the host nvidia driver's "nvidia/wine" dir that ships nvngx.dll / _nvngx.dll
// proton derives this as dirname(libGLX_nvidia.so.0)/nvidia/wine, but
// resolves the .so via dlopen and dlinfo, while we resolve it from the ldconfig cache instead
// https://github.com/ValveSoftware/Proton/blob/0745bfbc4cf4365e8cf048b003990c59def29948/proton#L376
fn find_nvidia_wine_dll_dir() -> Option<PathBuf> {
    if let Ok(output) = std::process::Command::new("ldconfig").arg("-p").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if !line.contains("libGLX_nvidia.so.0") {
                continue;
            }

            if let Some(path) = line.split("=>").nth(1)
                && let Ok(real) = PathBuf::from(path.trim()).canonicalize()
                && let Some(parent) = real.parent()
            {
                let dir = parent.join("nvidia").join("wine");

                if dir.join("nvngx.dll").exists() {
                    return Some(dir);
                }
            }
        }
    }

    None
}

// install vkd3d-proton into the prefix
// https://github.com/Winetricks/winetricks/blob/08304e81f9ac9a83c552a6bd78689040d174bf95/src/winetricks#L8279
async fn install_vkd3d(
    wine: &Wine,
    temp: &Path,
    system32: &Path,
    syswow64: &Path,
    progress_bar_input: &Sender<ProgressBarMsg>
) -> anyhow::Result<()> {
    let uri = latest_release_asset(VKD3D_REPO, ".tar.zst").await?;

    let extract_dir = temp.join("vkd3d-proton-extract");

    unpack_release(&uri, temp, &extract_dir, progress_bar_input).await?;

    let root = find_arch_root(&extract_dir)?;

    for dll in ["d3d12.dll", "d3d12core.dll"] {
        copy_dll(&root.join("x64").join(dll), system32)?;
        copy_dll(&root.join("x86").join(dll), syswow64)?;
    }

    for dll in ["d3d12", "d3d12core"] {
        wine.add_override(dll, [OverrideMode::Native])?;
    }

    let _ = std::fs::remove_dir_all(&extract_dir);

    Ok(())
}

// install dxvk-nvapi (nvidia nvapi implementation, needed for dlss/reflex)
// https://github.com/jp7677/dxvk-nvapi/blob/bfd44821a77fc591635ae0e56c0b0e49cb26d3a5/README.md#wine--wine-staging
async fn install_nvapi(
    wine: &Wine,
    temp: &Path,
    system32: &Path,
    syswow64: &Path,
    progress_bar_input: &Sender<ProgressBarMsg>
) -> anyhow::Result<()> {
    let uri = latest_release_asset(NVAPI_REPO, ".tar.gz").await?;

    let extract_dir = temp.join("dxvk-nvapi-extract");

    unpack_release(&uri, temp, &extract_dir, progress_bar_input).await?;

    let root = find_arch_root(&extract_dir)?;

    copy_dll(&root.join("x64").join("nvapi64.dll"), system32)?;
    copy_dll(&root.join("x64").join("nvofapi64.dll"), system32)?;
    copy_dll(&root.join("x32").join("nvapi.dll"), syswow64)?;

    for dll in ["nvapi", "nvapi64", "nvofapi64"] {
        wine.add_override(dll, [OverrideMode::Native])?;
    }

    // dxvk-nvapi needs the driver's nvngx dlls for dlss
    match find_nvidia_wine_dll_dir() {
        Some(nvidia_dir) => {
            for dll in ["nvngx.dll", "_nvngx.dll"] {
                let src = nvidia_dir.join(dll);

                if src.exists() && let Err(err) = copy_dll(&src, system32) {
                    tracing::warn!("Failed to copy {dll} from host nvidia driver: {err}");
                }
            }
        }

        None => tracing::warn!("Could not locate host nvidia driver nvngx dlls; DLSS will be unavailable")
    }

    let _ = std::fs::remove_dir_all(&extract_dir);

    Ok(())
}

pub fn install_dx12(
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
                            title: i18n!("dx12-install-failed"),
                            description: None
                        });

                        return;
                    };

                    let components_path = config.components.path.clone();
                    let prefix = config.game.wine.prefix.clone();

                    let wine = wine_config
                        .to_wine(
                            components_path.clone(),
                            Some(config.game.wine.builds.join(&wine_config.name)),
                        )
                        .with_prefix(prefix.clone())
                        .with_loader(WineLoader::Current);

                    let system32 = prefix.join("drive_c").join("windows").join("system32");
                    let syswow64 = prefix.join("drive_c").join("windows").join("syswow64");

                    let temp = config.launcher.temp.clone().unwrap_or_else(std::env::temp_dir);

                    sender.input(AppMsg::SetDownloading(true));

                    let result = runtime.block_on(async {
                        install_vkd3d(&wine, &temp, &system32, &syswow64, &progress_bar_input)
                            .await?;

                        install_nvapi(&wine, &temp, &system32, &syswow64, &progress_bar_input)
                            .await
                    });

                    sender.input(AppMsg::SetDownloading(false));

                    if let Err(err) = result {
                        tracing::error!("Failed to install DX12 support: {err}");

                        sender.input(AppMsg::Toast {
                            title: i18n!("dx12-install-failed"),
                            description: Some(err.to_string())
                        });
                    }

                    sender.input(AppMsg::DisableButtons(false));
                    sender.input(AppMsg::UpdateLauncherState {
                        perform_on_download_needed: false,
                        show_status_page: true
                    });
                }
            ));
        }

        Ok(None) => {
            tracing::error!("Failed to get selected wine executable");
            sender.input(AppMsg::Toast {
                title: i18n!("failed-get-selected-wine"),
                description: None
            });
        }

        Err(err) => {
            tracing::error!("Failed to get selected wine executable: {err}");
            sender.input(AppMsg::Toast {
                title: i18n!("failed-get-selected-wine"),
                description: Some(err.to_string())
            });
        }
    }
}

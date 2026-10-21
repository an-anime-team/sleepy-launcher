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

use anime_launcher_sdk::anime_game_core::minreq;
use anime_launcher_sdk::anime_game_core::installer::downloader::Downloader;

use crate::*;
use crate::ui::components::*;

use super::{App, AppMsg};

const VKD3D_REPO: &str = "HansKristian-Work/vkd3d-proton";
const NVAPI_REPO: &str = "jp7677/dxvk-nvapi";

// find the download url of the latest release asset whose name ends with `suffix`.
// we read the actual asset url instead of building it ourselves because upstream
// uses different compression
fn latest_release_asset(repo: &str, suffix: &str) -> anyhow::Result<String> {
    let response = minreq::get(format!("https://api.github.com/repos/{repo}/releases/latest"))
        .with_header("User-Agent", "sleepy-launcher")
        .with_timeout(15)
        .send()?;

    let json: serde_json::Value = serde_json::from_slice(response.as_bytes())?;

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

// get local filename from the download
fn url_filename<'a>(url: &'a str, fallback: &'a str) -> &'a str {
    url.rsplit('/').next().filter(|name| !name.is_empty()).unwrap_or(fallback)
}

// based on core downloader
fn download_file(uri: &str, output: &Path, progress_bar_input: &Sender<ProgressBarMsg>) -> anyhow::Result<()> {
    let mut downloader = Downloader::new(uri)?;

    downloader.download(output, clone!(
        #[strong] progress_bar_input,
        move |curr, total| {
            #[allow(unused_must_use)] {
                progress_bar_input.send(ProgressBarMsg::UpdateProgress(curr, total));
            }
        }
    )).map_err(|err| anyhow::anyhow!("{err}"))?;

    Ok(())
}

fn extract_archive(archive: &Path, dest: &Path) -> anyhow::Result<()> {
    let output = std::process::Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .output()?;

    if !output.status.success() {
        anyhow::bail!("failed to extract {}: {}", archive.display(), String::from_utf8_lossy(&output.stderr).trim());
    }

    Ok(())
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
fn install_vkd3d(
    wine: &Wine,
    temp: &Path,
    system32: &Path,
    syswow64: &Path,
    progress_bar_input: &Sender<ProgressBarMsg>
) -> anyhow::Result<()> {
    let uri = latest_release_asset(VKD3D_REPO, ".tar.zst")?;

    let archive = temp.join(url_filename(&uri, "vkd3d-proton.tar.zst"));
    let extract_dir = temp.join("vkd3d-proton-extract");

    #[allow(unused_must_use)] {
        progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(i18n!("downloading"))));
    }

    // start fresh so a previous failed attempt can't be resumed into
    let _ = std::fs::remove_file(&archive);

    download_file(&uri, &archive, progress_bar_input)?;

    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)?;
    extract_archive(&archive, &extract_dir)?;

    let root = find_arch_root(&extract_dir)?;

    for dll in ["d3d12.dll", "d3d12core.dll"] {
        copy_dll(&root.join("x64").join(dll), system32)?;
        copy_dll(&root.join("x86").join(dll), syswow64)?;
    }

    for dll in ["d3d12", "d3d12core"] {
        wine.add_override(dll, [OverrideMode::Native])?;
    }

    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_dir_all(&extract_dir);

    Ok(())
}

// install dxvk-nvapi (nvidia nvapi implementation, needed for dlss/reflex)
// https://github.com/jp7677/dxvk-nvapi/blob/bfd44821a77fc591635ae0e56c0b0e49cb26d3a5/README.md#wine--wine-staging
fn install_nvapi(
    wine: &Wine,
    temp: &Path,
    system32: &Path,
    syswow64: &Path,
    progress_bar_input: &Sender<ProgressBarMsg>
) -> anyhow::Result<()> {
    let uri = latest_release_asset(NVAPI_REPO, ".tar.gz")?;

    let archive = temp.join(url_filename(&uri, "dxvk-nvapi.tar.gz"));
    let extract_dir = temp.join("dxvk-nvapi-extract");

    #[allow(unused_must_use)] {
        progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(i18n!("downloading"))));
    }

    // start fresh so a previous failed attempt can't be resumed into
    let _ = std::fs::remove_file(&archive);

    download_file(&uri, &archive, progress_bar_input)?;

    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)?;
    extract_archive(&archive, &extract_dir)?;

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

    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_dir_all(&extract_dir);

    Ok(())
}

pub fn install_dx12(sender: ComponentSender<App>, progress_bar_input: Sender<ProgressBarMsg>) {
    let config = Config::get().unwrap();

    match config.get_selected_wine() {
        Ok(Some(wine_config)) => {
            sender.input(AppMsg::DisableButtons(true));

            std::thread::spawn(clone!(
                #[strong] sender,
                move || {
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

                    let result = install_vkd3d(&wine, &temp, &system32, &syswow64, &progress_bar_input)
                        .and_then(|()| install_nvapi(&wine, &temp, &system32, &syswow64, &progress_bar_input));

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

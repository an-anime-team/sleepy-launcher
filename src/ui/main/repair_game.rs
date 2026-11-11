use std::sync::atomic::AtomicU64;

use anime_launcher_sdk::anime_game_core::sophon::api::{ApiContext, ApiContextWrapper};
use anime_launcher_sdk::anime_game_core::sophon::api::schemas::sophon_manifests::SophonDownloadInfo;
use relm4::Sender;
use anime_launcher_sdk::anime_game_core::sophon::installer::{
    SophonInstaller, Update as SophonRepairerUpdate
};

use crate::*;
use crate::ui::components::*;
use super::{App, AppMsg};

#[allow(unused_must_use)]
pub fn repair_game(sender: ComponentSender<App>, progress_bar_input: Sender<ProgressBarMsg>) {
    let config = Config::get().expect("failed to read launcher config");

    progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(i18n!(
        "verifying-files"
    ))));
    sender.input(AppMsg::SetDownloading(true));

    std::thread::spawn(move || {
        let api = ApiContext::new(config.launcher.edition.into());

        let manifest = match get_download_manifest(&api, config.launcher.edition) {
            Ok(manifest) => manifest,
            Err(err) => {
                tracing::error!("Faield to get download info: {err}");
                sender.input(AppMsg::Toast {
                    title: "Repair error".to_owned(),
                    description: Some(err.to_string())
                });
                return;
            }
        };

        let game_path = config.game.path.for_edition(&config.launcher.edition);

        let repairer_temp = config.launcher.temp.unwrap_or_else(std::env::temp_dir);
        let mut repairer = match SophonInstaller::new(manifest.as_ref(), &repairer_temp) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Error initializing repairer: {e:?}");
                sender.input(AppMsg::Toast {
                    title: "Repair error".to_owned(),
                    description: Some(e.to_string())
                });
                return;
            }
        };
        repairer.mode_repair = true;
        repairer.inplace = true;
        repairer.chunks_in_mem = true;
        repairer.chunks_queue_data_limit = Some(2 * 1024 * 1024 * 1024); // 2GiB

        let total_files_to_repair = AtomicU64::new(0);
        let total_to_repair = &total_files_to_repair;

        let updater = |msg: SophonRepairerUpdate| match msg {
            SophonRepairerUpdate::CheckingFilesProgress {
                total,
                passed
            } => {
                tracing::trace!(passed, total, "Verification progress");

                progress_bar_input.send(ProgressBarMsg::UpdateProgressCounter(passed, total));
            }

            SophonRepairerUpdate::DownloadingProgressFiles {
                downloaded_files, ..
            } => {
                tracing::trace!(
                    downloaded_files,
                    total_to_repair = total_to_repair.load(Ordering::Acquire),
                    "Repairing progress"
                );

                progress_bar_input.send(ProgressBarMsg::UpdateProgressCounter(
                    downloaded_files,
                    total_to_repair.load(Ordering::Acquire)
                ));
            }

            SophonRepairerUpdate::CheckingFiles {
                ..
            } => {
                tracing::trace!("Verification started");
            }

            SophonRepairerUpdate::DownloadingStarted {
                total_files, ..
            } => {
                tracing::trace!("Repairing started");

                total_to_repair.store(total_files, Ordering::Release);

                progress_bar_input.send(ProgressBarMsg::UpdateCaption(Some(i18n!(
                    "repairing-files"
                ))));
            }

            SophonRepairerUpdate::DownloadingFinished => {
                tracing::trace!("Repair finished");
            }

            SophonRepairerUpdate::DownloadingError(err) => {
                tracing::error!(?err, "Error during repairing")
            }

            _ => {}
        };

        repairer.install(
            game_path,
            config.launcher.repairer.threads as usize,
            Box::new(updater)
        );

        let _ = std::fs::remove_dir_all(repairer.downloading_temp());

        sender.input(AppMsg::SetDownloading(false));
    });
}

fn get_download_manifest<'ctx>(
    api: &'ctx ApiContext,
    edition: GameEdition
) -> anyhow::Result<ApiContextWrapper<'ctx, SophonDownloadInfo>> {
    let downloads = api
        .game_branches()?
        .as_ref()
        .package_latest(edition.api_game_id(), false)
        .ok_or_else(|| anyhow::anyhow!("Failed to find latest game package"))?
        .downloads()?;

    let game_download_info = downloads
        .as_ref()
        .for_component("game")
        .ok_or_else(|| anyhow::anyhow!("Failed to find download info for the game"))?;

    Ok(game_download_info.cloned())
}

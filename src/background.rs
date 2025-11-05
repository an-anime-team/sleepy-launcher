use std::process::Command;

use anime_launcher_sdk::anime_game_core::installer::downloader::Downloader;
use anime_launcher_sdk::anime_game_core::minreq;
use md5::{Digest, Md5};

#[derive(Debug, Clone)]
pub struct ComposedBackground {
    pub background: Background,
    pub overlay: Background
}

#[derive(Debug, Clone)]
pub struct Background {
    pub uri: String,
    pub hash: String
}

pub fn get_uri() -> String {
    let lang = crate::i18n::get_lang();

    if lang.language == unic_langid::langid!("zh-cn").language {
        concat!(
            "https://hyp-api.",
            "mi",
            "ho",
            "yo",
            ".com/hyp/hyp-connect/api/getAllGameBasicInfo?launcher_id=jGHBHlcOq1"
        )
        .to_owned()
    }
    else {
        let uri = concat!(
            "https://sg-hyp-api.",
            "ho",
            "yo",
            "verse",
            ".com/hyp/hyp-connect/api/getAllGameBasicInfo?launcher_id=VYTpXlbWo8&language="
        );

        uri.to_owned() + &crate::i18n::format_lang(lang)
    }
}

#[cached::proc_macro::cached(result)]
pub fn get_background_info() -> anyhow::Result<ComposedBackground> {
    let json =
        serde_json::from_slice::<serde_json::Value>(minreq::get(get_uri()).send()?.as_bytes())?;

    let backgrounds_info = json["data"]["game_info_list"].as_array()
        .ok_or_else(|| anyhow::anyhow!("Failed to list games in the backgrounds API"))?
        .iter()
        .find(|game| {
            match game["game"]["biz"].as_str() {
                Some(biz) => biz.starts_with("nap_"),
                _ => false
            }
        })
        .ok_or_else(|| anyhow::anyhow!("Failed to find the game in the backgrounds API"))?["backgrounds"]
        .as_array()
        .and_then(|backgrounds| backgrounds.iter().next());

    let background_uri = backgrounds_info
        .and_then(|background| background["background"]["url"].as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get background picture url"))?
        .to_string();
    let overlay_uri = backgrounds_info
        .and_then(|background| background["theme"]["url"].as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get overlay picture url"))?
        .to_string();

    let background_hash = get_img_hash_from_uri(&background_uri);
    let overlay_hash = get_img_hash_from_uri(&overlay_uri);

    Ok(ComposedBackground {
        background: Background {
            uri: background_uri,
            hash: background_hash
        },
        overlay: Background {
            uri: overlay_uri,
            hash: overlay_hash
        }
    })
}

fn get_img_hash_from_uri(uri: &str) -> String {
    uri.split('/')
        .next_back()
        .unwrap_or_default()
        .split('_')
        .next()
        .unwrap_or_default()
        .to_owned()
}

pub fn download_background() -> anyhow::Result<()> {
    tracing::debug!("Downloading background picture");

    let info = get_background_info()?;

    let mut download_image = true;
    let mut download_overlay = true;

    if crate::BACKGROUND_FILE.exists() {
        let hash = Md5::digest(std::fs::read(crate::BACKGROUND_FILE.as_path())?);
        let info = &info.background;

        if format!("{:x}", hash).to_lowercase() == info.hash {
            tracing::debug!("Background picture is already downloaded. Skipping");

            download_image = false;
        }
    }

    if crate::BACKGROUND_OVERLAY_FILE.exists() {
        let hash = Md5::digest(std::fs::read(crate::BACKGROUND_OVERLAY_FILE.as_path())?);
        let info = &info.overlay;

        if format!("{:x}", hash).to_lowercase() == info.hash {
            tracing::debug!("Background picture is already downloaded. Skipping");

            download_overlay = false;
        }
    }

    if download_image {
        let mut downloader = Downloader::new(&info.background.uri)?;

        downloader.continue_downloading = false;

        if let Err(err) = downloader.download(crate::BACKGROUND_FILE.as_path(), |_, _| {}) {
            anyhow::bail!(err);
        }
    }

    if download_overlay {
        let mut downloader = Downloader::new(&info.overlay.uri)?;

        downloader.continue_downloading = false;

        if let Err(err) = downloader.download(crate::BACKGROUND_OVERLAY_FILE.as_path(), |_, _| {}) {
            anyhow::bail!(err);
        }
    }

    Command::new("magick")
        .arg(crate::BACKGROUND_FILE.as_path())
        .arg(crate::BACKGROUND_OVERLAY_FILE.as_path())
        .args(["-layers", "flatten"])
        .arg(format!(
            "PNG:{}",
            crate::PROCESSED_BACKGROUND_FILE.display()
        ))
        .spawn()?
        .wait()?;

    // If it failed to re-code the file - just copy it
    // Will happen with HSR because devs apparently named
    // their background image ".webp" while it's JPEG
    if !crate::PROCESSED_BACKGROUND_FILE.exists() {
        std::fs::copy(
            crate::BACKGROUND_FILE.as_path(),
            crate::PROCESSED_BACKGROUND_FILE.as_path()
        )?;
    }

    Ok(())
}

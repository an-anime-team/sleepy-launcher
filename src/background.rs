use std::path::Path;
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

fn get_img_hash_from_uri(uri: &str) -> String {
    uri.split('/')
        .next_back()
        .unwrap_or_default()
        .split('_')
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn get_img_uri_from_json_value(
    backgrounds_info: Option<&serde_json::Value>,
    key: &str
) -> anyhow::Result<String> {
    Ok(backgrounds_info
        .and_then(|background| background[key]["url"].as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get background picture url"))?
        .to_string())
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

    let background_uri = get_img_uri_from_json_value(backgrounds_info, "background")?;
    let overlay_uri = get_img_uri_from_json_value(backgrounds_info, "theme")?;

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

/// Returns true if image exists and is correct
fn check_img_file(path: &Path, expected_hash: &str) -> anyhow::Result<bool> {
    if path.exists() {
        let hash = Md5::digest(std::fs::read(path)?);

        if format!("{hash:x}").eq_ignore_ascii_case(expected_hash) {
            tracing::debug!("Background picture {path:?} already downloaded. Skipping");

            return Ok(true);
        }
    }

    Ok(false)
}

fn download_img_file(path: &Path, uri: &str) -> anyhow::Result<()> {
    let mut downloader = Downloader::new(uri)?;

    downloader.continue_downloading = false;

    if let Err(err) = downloader.download(path, |_, _| {}) {
        anyhow::bail!(err);
    }

    Ok(())
}

pub fn download_background() -> anyhow::Result<()> {
    tracing::debug!("Downloading background picture");

    let info = get_background_info()?;

    let mut regenerate_image = false;

    if !check_img_file(&crate::BACKGROUND_FILE, &info.background.hash)? {
        download_img_file(&crate::BACKGROUND_FILE, &info.background.uri)?;
        regenerate_image = true;
    }

    if !check_img_file(&crate::BACKGROUND_OVERLAY_FILE, &info.overlay.hash)? {
        download_img_file(&crate::BACKGROUND_OVERLAY_FILE, &info.overlay.uri)?;
        regenerate_image = true;
    }

    if regenerate_image {
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
    }
    else {
        tracing::debug!("Not re-generating the background image, already latest")
    }

    Ok(())
}

use std::path::Path;
use std::process::Command;

use anime_launcher_sdk::anime_game_core::installer::downloader::Downloader;
use anime_launcher_sdk::anime_game_core::minreq;
use md5::{Digest, Md5};

#[derive(Debug, Clone)]
pub enum BackgroundSpec {
    Normal {
        background: Background
    },
    Video {
        background: Background,
        video: Background,
        overlay: Background
    }
}

impl BackgroundSpec {
    fn from_json(value: &serde_json::Value) -> anyhow::Result<Self> {
        let backgrounds_info =
            value["data"]["game_info_list"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Failed to list games in the backgrounds API"))?
                .iter()
                .find(|game| match game["game"]["biz"].as_str() {
                    Some(biz) => biz.starts_with("nap_"),
                    _ => false
                })
                .ok_or_else(|| anyhow::anyhow!("Failed to find the game in the backgrounds API"))?
                ["backgrounds"]
                .as_array()
                .and_then(|backgrounds| backgrounds.iter().next_back());

        let background_uri = get_img_uri_from_json_value(backgrounds_info, "background")?;
        let background = Background::from_uri(background_uri);

        if backgrounds_info.and_then(|bginfo| bginfo["type"].as_str())
            == Some("BACKGROUND_TYPE_VIDEO")
        {
            let video_uri = get_img_uri_from_json_value(backgrounds_info, "video")?;
            let video = Background::from_uri(video_uri);

            let overlay_uri = get_img_uri_from_json_value(backgrounds_info, "theme")?;
            let overlay = Background::from_uri(overlay_uri);

            Ok(Self::Video {
                background,
                video,
                overlay
            })
        }
        else {
            Ok(Self::Normal {
                background
            })
        }
    }

    fn background(&self) -> &Background {
        match self {
            Self::Normal {
                background
            }
            | Self::Video {
                background, ..
            } => background
        }
    }

    /// Return value indicates whether the background needs to be re-generated
    fn download(&self) -> anyhow::Result<bool> {
        let mut regenerate_image = false;

        regenerate_image |= self.background().download(&crate::BACKGROUND_FILE)?;

        if let Self::Video {
            video,
            overlay,
            ..
        } = self
        {
            regenerate_image |= overlay.download(&crate::BACKGROUND_OVERLAY_FILE)?;
            regenerate_image |= video.download(&crate::BACKGROUND_VIDEO_FILE)?;
        }

        Ok(regenerate_image)
    }
}

#[derive(Debug, Clone)]
pub struct Background {
    pub uri: String,
    pub hash: String
}

impl Background {
    fn from_uri(uri: String) -> Self {
        let hash = get_img_hash_from_uri(&uri);
        Self {
            uri,
            hash
        }
    }

    /// Return value indicates whether the background needs to be re-generated
    fn download(&self, path: &Path) -> anyhow::Result<bool> {
        if !check_img_file(path, &self.hash)? {
            download_img_file(path, &self.uri)?;
            return Ok(true);
        }
        Ok(false)
    }
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

#[cached::proc_macro::once()]
fn gtk_webp_image_supported() -> bool {
    let supported_pixbuf_formats = gtk::gdk_pixbuf::Pixbuf::formats();
    supported_pixbuf_formats.into_iter().any(|format| {
        format
            .name()
            .map(|name| name.eq_ignore_ascii_case("webp"))
            .unwrap_or(false)
            || format
                .extensions()
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case("webp"))
    })
}

#[cached::proc_macro::cached(result)]
pub fn get_background_info() -> anyhow::Result<BackgroundSpec> {
    let json =
        serde_json::from_slice::<serde_json::Value>(minreq::get(get_uri()).send()?.as_bytes())?;

    BackgroundSpec::from_json(&json)
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

    let regenerate_image = info.download()?;

    if regenerate_image {
        if matches!(info, BackgroundSpec::Video { .. }) {
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
        }
        else {
            Command::new("magick")
                .arg(crate::BACKGROUND_FILE.as_path())
                .arg(format!(
                    "PNG:{}",
                    crate::PROCESSED_BACKGROUND_FILE.display()
                ))
                .spawn()?
                .wait()?;
        }

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

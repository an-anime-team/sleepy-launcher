use std::path::{Path, PathBuf};

use anime_launcher_sdk::config::ConfigExt;
use anime_launcher_sdk::zzz::config::{Config, Schema};

const NVIDIA_WINE_DLLS: [&str; 2] = ["nvngx.dll", "_nvngx.dll"];
const DLSS_DLL_OVERRIDES: [(&str, &str); 4] = [
    ("nvcuda", "b"),
    ("msasn1", "n,b"),
    ("wintrust", "b"),
    ("crypt32", "b")
];

fn is_nvidia_wine_dll_dir(path: &Path) -> bool {
    NVIDIA_WINE_DLLS.iter().any(|dll| path.join(dll).is_file())
}

fn canonical_nvidia_wine_dll_dir(path: impl Into<PathBuf>) -> Option<PathBuf> {
    let path = path.into();

    if !is_nvidia_wine_dll_dir(&path) {
        return None;
    }

    Some(path.canonicalize().unwrap_or(path))
}

/// Locate the NVIDIA driver's Wine DLL directory containing nvngx.dll and/or
/// _nvngx.dll. Proton derives this directory from libGLX_nvidia.so.0; checking
/// the dynamic linker cache gives us the same location without loading the
/// driver into the launcher process.
pub fn find_nvidia_wine_dll_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("NVIDIA_WINE_DLL_DIR") {
        if let Some(path) = canonical_nvidia_wine_dll_dir(path) {
            return Some(path);
        }
    }

    for path in [
        "/usr/lib/nvidia/wine",
        "/usr/lib64/nvidia/wine",
        "/usr/lib/x86_64-linux-gnu/nvidia/wine"
    ] {
        if let Some(path) = canonical_nvidia_wine_dll_dir(path) {
            return Some(path);
        }
    }

    for ldconfig in ["ldconfig", "/sbin/ldconfig", "/usr/sbin/ldconfig"] {
        let Ok(output) = std::process::Command::new(ldconfig).arg("-p").output()
        else {
            continue;
        };

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if !line.contains("libGLX_nvidia.so.0") {
                continue;
            }

            let Some(path) = line.split("=>").nth(1)
            else {
                continue;
            };

            let library = PathBuf::from(path.trim());
            let library = library.canonicalize().unwrap_or(library);

            if let Some(parent) = library.parent() {
                if let Some(path) = canonical_nvidia_wine_dll_dir(parent.join("nvidia/wine")) {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn override_dll_name(entry: &str) -> Option<&str> {
    let name = entry.split_once('=')?.0.trim().trim_start_matches('*');
    let extension = name.get(name.len().saturating_sub(4)..);

    if extension.is_some_and(|extension| extension.eq_ignore_ascii_case(".dll")) {
        name.get(..name.len() - 4)
    }
    else {
        Some(name)
    }
}

fn set_wine_dll_override(overrides: Option<&str>, dll: &str, mode: &str) -> String {
    let mut result = Vec::new();
    let mut replaced = false;

    for entry in overrides
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        if override_dll_name(entry).is_some_and(|name| name.eq_ignore_ascii_case(dll)) {
            if !replaced {
                result.push(format!("{dll}={mode}"));
                replaced = true;
            }
        }
        else {
            result.push(entry.to_string());
        }
    }

    if !replaced {
        result.push(format!("{dll}={mode}"));
    }

    result.join(";")
}

fn is_native_windows_dll_contents(contents: &[u8]) -> bool {
    contents.starts_with(b"MZ")
        && !contents
            .windows(b"Wine builtin DLL".len())
            .any(|window| window == b"Wine builtin DLL")
}

fn is_native_windows_dll(path: &Path) -> bool {
    std::fs::read(path)
        .map(|contents| is_native_windows_dll_contents(&contents))
        .unwrap_or(false)
}

fn configure_dlss_environment(config: &mut Schema) -> bool {
    if !config.game.enhancements.dx12 {
        return false;
    }

    let original = config.game.environment.clone();
    let runner_overrides = config
        .get_selected_wine()
        .ok()
        .flatten()
        .and_then(|wine| wine.features(&config.components.path).ok().flatten())
        .and_then(|features| features.env.get("WINEDLLOVERRIDES").cloned());
    let environment = &mut config.game.environment;

    let configured_nvidia_dir = environment
        .get("NVIDIA_WINE_DLL_DIR")
        .and_then(|path| canonical_nvidia_wine_dll_dir(path));

    if configured_nvidia_dir.is_none() {
        if let Some(path) = find_nvidia_wine_dll_dir() {
            tracing::info!("Using NVIDIA Wine DLL directory: {}", path.display());

            environment.insert(
                "NVIDIA_WINE_DLL_DIR".to_string(),
                path.to_string_lossy().into_owned()
            );
        }
        else {
            tracing::warn!("Could not locate NVIDIA Wine DLLs; DLSS will be unavailable");
        }
    }

    // Keep explicit user choices, but make the working DX12/NVAPI/HAGS values
    // the default. The HAGS variable is understood by patched Spritz builds and
    // is harmless on runners that do not implement the query yet.
    environment
        .entry("DXVK_ENABLE_NVAPI".to_string())
        .or_insert_with(|| "1".to_string());
    environment
        .entry("WINE_DISABLE_HARDWARE_SCHEDULING".to_string())
        .or_insert_with(|| "0".to_string());

    let mut overrides = environment
        .get("WINEDLLOVERRIDES")
        .cloned()
        .or(runner_overrides)
        .or_else(|| std::env::var("WINEDLLOVERRIDES").ok());

    for (dll, mode) in DLSS_DLL_OVERRIDES {
        overrides = Some(set_wine_dll_override(overrides.as_deref(), dll, mode));
    }

    environment.insert("WINEDLLOVERRIDES".to_string(), overrides.unwrap());

    let msasn1 = config
        .game
        .wine
        .prefix
        .join("drive_c/windows/system32/msasn1.dll");

    let uses_spritz = config
        .game
        .wine
        .selected
        .as_deref()
        .is_some_and(|runner| runner.to_ascii_lowercase().contains("spritz"));

    if uses_spritz && !is_native_windows_dll(&msasn1) {
        tracing::warn!(
            "Native x64 msasn1.dll is not installed at {}; NVIDIA Streamline signature validation may fail",
            msasn1.display()
        );
    }

    config.game.environment != original
}

/// Run ZZZ with the DLSS environment that Proton normally prepares around its
/// Wine build. Save generated variables in config.json so every SDK launch path
/// and future launcher session inherits the same working configuration.
pub fn run_game() -> anyhow::Result<bool> {
    let mut config = Config::get()?;

    if configure_dlss_environment(&mut config) {
        Config::update_raw(config)?;
    }

    anime_launcher_sdk::zzz::game::run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_override_without_losing_existing_values() {
        assert_eq!(
            set_wine_dll_override(Some("d3d12=n;dxgi=n,b"), "nvcuda", "b"),
            "d3d12=n;dxgi=n,b;nvcuda=b"
        );
    }

    #[test]
    fn replaces_disabled_override_and_removes_duplicates() {
        assert_eq!(
            set_wine_dll_override(Some("NVCUDA=disabled;d3d12=n;nvcuda.dll=n"), "nvcuda", "b"),
            "nvcuda=b;d3d12=n"
        );
    }

    #[test]
    fn recognizes_wildcard_dll_names() {
        assert_eq!(
            set_wine_dll_override(Some("*MSASN1.DLL=b;dxgi=n"), "msasn1", "n,b"),
            "msasn1=n,b;dxgi=n"
        );
    }

    #[test]
    fn distinguishes_wine_builtin_marker() {
        assert!(is_native_windows_dll_contents(
            b"MZ\x00\x00native windows dll"
        ));
        assert!(!is_native_windows_dll_contents(
            b"MZ\x00Wine builtin DLL\x00"
        ));
        assert!(!is_native_windows_dll_contents(b"not a PE file"));
    }
}

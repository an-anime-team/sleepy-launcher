# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

# [1.6.1] - 2025-12-15

### Added

- Added support for animated backgrounds (can be turned off in the settings)
- Added support for selecting the background via config file if there are multiple

### Fixed

- Fixed signal search error message in english

### Removed

- Removed imagemagick dependency

## [1.6.0] - 12.11.2025

### Added

- Added support for layered launcher backgrounds (background separate from text)

### Fixed

- Fixed size of images in the "Appearance" preferences section (most noticeable on flatpak)
- Updated default window size in classic appearance option to match the other launchers

### Changed

- Removed dwebp dependency, replaced by imagemagick

## [1.5.0] - 22.09.2025

### Added

- Added DXVK installation check for broken dxvk installations

## [1.4.0] - 13.09.2025

### Added

- Added ability to extract the signal search history URL

### Changed

- Removed support for launching with Proton from the launcher.
  Launching with proton externally is unaffected.

## [1.3.0] - 09.10.2024

### Removed

- Removed Discord RPC support

## [1.2.1] - 02.09.2024

### Fixed

- Fixed gamescope config file layout

## [1.2.0] - 01.09.2024

### Added

- Apply chmod 755 to extracted files if 7z was used

### Changed

- Reworked gamescope settings

### Fixed

- Create cache folder if it doesn't exist
- (potentially) fixed a bug with pre-download button
- Fixed calculation of unpacked files size due to API changes
- Respect downloaded file size in free space check

## [1.1.0] - 16.08.2024

### Added

- Respect root `.version` file for game version parsing
- Added disabling of 2 new telemetry servers

### Fixed

- Create cache folder if it doesn't exist

### Changed

- Prioritize parsed game version over the API response

### Removed

- Removed migrate installation feature

## [1.0.1] - 05.07.2024

### Fixed

- Fixed infinite updates loop on minor game patches (notably 1.0.1)
- Fixed prefix paths for proton builds for game drives mapping

### Changed

- Removed xdelta3 runtime dependency, updated dwebp package name for fedora

## [1.0.0] - 04.07.2024

🚀 Initial release

<br>

[unreleased]: https://github.com/an-anime-team/sleepy-launcher/compare/1.6.1...next
[1.6.1]: https://github.com/an-anime-team/sleepy-launcher/compare/1.6.0...1.6.1
[1.6.0]: https://github.com/an-anime-team/sleepy-launcher/compare/1.5.0...1.6.0
[1.5.0]: https://github.com/an-anime-team/sleepy-launcher/compare/1.4.0...1.5.0
[1.4.0]: https://github.com/an-anime-team/sleepy-launcher/compare/1.3.0...1.4.0
[1.3.0]: https://github.com/an-anime-team/sleepy-launcher/compare/1.2.1...1.3.0
[1.2.1]: https://github.com/an-anime-team/sleepy-launcher/compare/1.2.0...1.2.1
[1.2.0]: https://github.com/an-anime-team/sleepy-launcher/compare/1.1.0...1.2.0
[1.1.0]: https://github.com/an-anime-team/sleepy-launcher/compare/1.0.1...1.1.0
[1.0.1]: https://github.com/an-anime-team/sleepy-launcher/compare/1.0.0...1.0.1
[1.0.0]: https://github.com/an-anime-team/sleepy-launcher/releases/tag/1.0.0

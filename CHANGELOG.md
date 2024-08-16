# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Respect root `.version` file for game version parsing
- Added disabling of 2 new telemetry servers

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

[unreleased]: https://github.com/an-anime-team/sleepy-launcher/compare/1.0.1...next
[1.0.1]: https://github.com/an-anime-team/sleepy-launcher/compare/1.0.0...1.0.1
[1.0.0]: https://github.com/an-anime-team/sleepy-launcher/releases/tag/1.0.0

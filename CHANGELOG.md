# Changelog

All notable changes to this project will be documented in this file.

## [1.0.0] - 2026-06-11
### Added
- **WSL2 Disk Shrinker**: Scans registered distributions, toggles sparse drive property, and executes safe `diskpart` compaction with a live Terminal UI logs viewer.
- **Toolchain Version Sweeper**: Detects NVM, FNM, and Rustup runtimes, calculates sizes, identifies active compiler versions, and performs clean uninstallation.
- **Universal Project Sweeper**: Recursively scans configured folders for heavy dependencies (`node_modules`, `target`, `venv`, etc.) and batch sweeps them.
- **Bulk Silent Uninstaller**: Decoupled registry scan, UWP package manager, and remnant cleaning with silent fallback.
- **PATH Cleaner**: Scans and sanitizes Windows environment variables to remove broken or duplicate entries.

### Fixed
- **Improved Responsiveness**: Fixed vertical and horizontal overflow layouts to ensure full usability in compact, windowed multitasking modes.
- **UI Bugfix**: Fixed invalid nesting of closing div tags in Dev Tools panel.

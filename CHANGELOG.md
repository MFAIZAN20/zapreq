## [0.1.8] - 2026-07-18

### Changed
- Refined progress handling and consolidated shared request, security, and UI utilities for improved maintainability.

## [0.1.7] - 2026-06-27

### Changed
- Finalized the `v0.1.7` release metadata after fixing header editor maintainability issues and extending header validation test coverage.

## [0.1.5] - 2026-06-19

### Changed
- Fixed release and AUR packaging to publish the `zapreq` binary instead of the stale `http` path.
- Aligned the Rust crate, Tauri app, and desktop UI version metadata for the 0.1.5 release.

### Removed
- Deleted unused frontend starter assets that were no longer referenced by the Tauri UI.

## [0.1.0] - 2026-05-09

### Added
- Full HTTPie-compatible CLI (METHOD URL ITEMS)
- JSON / XML / HTML / binary response formatting
- Syntax-highlighted output with 4 built-in themes
- Basic, Bearer, Digest (RFC 7616) authentication
- Named sessions with cookie + header persistence
- Environment profiles (~/.config/zapreq/envs/)
- Request collections (save/run/list/delete)
- AI-powered request assistant (ZAPREQ_AI_KEY)
- Response diffing (zapreq diff URL_A URL_B)
- Progress bar download with resume support
- Plugin system foundation
- Multi-platform: Linux, macOS, Windows

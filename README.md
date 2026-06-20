# zapreq

[![Crates.io](https://img.shields.io/crates/v/zapreq.svg)](https://crates.io/crates/zapreq)
[![CI](https://github.com/MFAIZAN20/zapreq/actions/workflows/ci.yml/badge.svg)](https://github.com/MFAIZAN20/zapreq/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

`zapreq` is a Rust-based HTTP client for API workflows. It provides a fast command-line interface for sending requests, plus local tooling for saved requests, environment profiles, regression testing, security scanning, performance benchmarking, notes, a terminal UI, and a Tauri desktop app.

## Highlights

- Fast native CLI with HTTPie-style request syntax
- Support for JSON, forms, multipart uploads, sessions, and authentication
- Saved requests, collections, and environment profiles for repeatable workflows
- Built-in regression testing, response diffing, performance benchmarks, and security checks
- Local-first tooling with TUI and desktop interfaces on top of the same core engine

## Installation

### Cargo

```bash
cargo install zapreq
```

### Prebuilt Binaries

Release binaries for Linux, macOS, and Windows are available on the [GitHub releases page](https://github.com/MFAIZAN20/zapreq/releases).

## Quick Start

```bash
# Simple request
zapreq GET https://httpbin.org/get

# JSON body fields
zapreq POST https://httpbin.org/post name="Faizan" active:=true

# Custom headers and query parameters
zapreq GET https://httpbin.org/get X-API-Token:secret page==1 limit==20

# Persist a session
zapreq --session dev-user POST https://api.example.com/login username=faizan password=secret

# Save and rerun a request
zapreq save get-user GET https://api.example.com/users/1
zapreq run get-user
```

## Request Syntax

`zapreq` interprets request items by operator:

| Operator | Meaning | Example |
| --- | --- | --- |
| `:` | Header | `Authorization:Bearer token` |
| `=` | String field | `name=faizan` |
| `:=` | Raw JSON field | `active:=true` |
| `==` | Query parameter | `page==2` |
| `@` | File upload | `avatar@/tmp/avatar.png` |
| `=@` | String from file | `body=@./payload.txt` |
| `:=@` | JSON from file | `config:=@./payload.json` |

Run `zapreq --help` for the full CLI surface.

## Core Commands

```bash
# Saved requests
zapreq save <alias> <request...>
zapreq run <alias>
zapreq list

# Collections and request workspaces
zapreq collections list
zapreq requests save <workspace> <name> -- <request...>
zapreq requests run <workspace> <request>

# Validation and analysis
zapreq test --expect-status 200 -- GET https://httpbin.org/get
zapreq regression run <suite>
zapreq security scan --alias <saved-request>
zapreq perf benchmark --alias <saved-request>
zapreq diff <url-a> <url-b>
```

## Development

### Prerequisites

- Rust stable toolchain
- Node.js for the desktop frontend

### Common Commands

```bash
# Run the CLI locally
cargo run -- --help

# Run tests
cargo test

# Frontend dependencies
cd frontend && npm install

# Start the desktop app from the repository root
npm run dev
```

## Project Structure

- `src/`: core CLI, request engine, testing, security, storage, and TUI code
- `frontend/`: React frontend for the desktop app
- `src-tauri/`: Tauri host application
- `tests/`: integration and CLI tests

## Contributing

Contributions are welcome. For documentation, bug fixes, and feature work:

1. Create a branch from `main`.
2. Make focused changes with tests where applicable.
3. Run `cargo test` before opening a pull request.

## License

Licensed under either of the following, at your option:

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

# zapreq

> A fast, friendly HTTP client for the terminal — HTTPie reimagined in Rust.

[![Crates.io](https://img.shields.io/crates/v/zapreq.svg)](https://crates.io/crates/zapreq)
[![CI](https://github.com/MFAIZAN20/zapreq/actions/workflows/ci.yml/badge.svg)](https://github.com/MFAIZAN20/zapreq/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)

`zapreq` installs an `http` binary. If you know HTTPie, you already know zapreq — same
syntax, faster startup, lower memory, and a handful of features HTTPie never shipped.

---

## Table of contents

- [Why zapreq](#why-zapreq)
- [Install](#install)
- [Quick start](#quick-start)
- [Request items](#request-items)
- [Output control](#output-control)
- [Authentication](#authentication)
- [Sessions](#sessions)
- [Environment profiles](#environment-profiles)
- [Request collections](#request-collections)
- [Terminal workspace](#terminal-workspace)
- [API testing](#api-testing)
- [Secrets](#secrets)
- [AI assistant](#ai-assistant)
- [Response diffing](#response-diffing)
- [Download mode](#download-mode)
- [Configuration](#configuration)
- [Plugins](#plugins)
- [Comparison with HTTPie](#comparison-with-httpie)
- [Contributing](#contributing)
- [License](#license)

---

## Why zapreq

- **~3 MB binary, ~5 ms startup.** HTTPie ships ~15 MB and takes ~200 ms. ZapReq is
  compiled Rust — no interpreter, no import overhead.
- **HTTPie-compatible syntax.** Every `key=value`, `key:=value`, `key==value` item works
  exactly as you expect.
- **Full auth coverage.** Basic, Bearer, and Digest (RFC 7616, MD5 + SHA-256) with an
  automatic 401 retry cycle for Digest.
- **Session persistence.** Cookies, headers, and auth are saved to
  `~/.config/zapreq/sessions/` and restored on the next request.
- **Environment profiles.** Switch between `dev`, `staging`, and `prod` with one flag.
- **Request collections.** Save a request by name, replay it later — Postman-style,
  entirely in the terminal.
- **AI assistant.** Describe a request in plain English; zapreq generates a concrete
  command and runs it only when you opt in.
- **Response diffing.** Compare two endpoints side by side, key by key.
- **Zero Python dependency.** One binary, no virtualenv, no pip.

---

## Install

```bash
cargo install zapreq
```

Pre-built binaries for Linux, macOS (x86 + ARM), and Windows are attached to every
[GitHub release](https://github.com/MFAIZAN20/zapreq/releases). Download and place
the binary on your `PATH` if you do not have a Rust toolchain.

**Package managers**

```bash
# Arch Linux (AUR)
yay -S zapreq

# macOS / Linux (Homebrew tap)
brew tap MFAIZAN20/zapreq
brew install zapreq
```

---

## Quick start

```bash
# GET request
http GET https://httpbin.org/get

# POST with JSON body (inferred from data items)
http POST https://httpbin.org/post name=faizan age:=22

# Form-encoded body
http --form POST https://httpbin.org/post field=value

# Custom headers
http GET https://httpbin.org/headers Accept:application/json X-Token:abc123

# Query parameters
http GET https://httpbin.org/get page==2 limit==10

# Basic auth
http --auth user:pass https://httpbin.org/basic-auth/user/pass

# Bearer token
http --auth-type bearer --auth "$TOKEN" https://api.example.com/me

# Named session (saves cookies + auth for next time)
http --session myapi POST https://api.example.com/login username=faizan password=secret
http --session myapi GET  https://api.example.com/me

# Download a file with a progress bar
http --download https://example.com/archive.zip

# Compare two API versions
http diff https://api.example.com/v1/user/1 https://api.example.com/v2/user/1
```

---

## Request items

Items are positional arguments after the URL. The operator between key and value
determines what the item does.

| Operator | Type | Example |
|---|---|---|
| `key:value` | Request header | `Accept:application/json` |
| `key=value` | String body field | `name=faizan` |
| `key:=value` | Raw JSON body field | `active:=true` or `count:=42` |
| `key==value` | Query string parameter | `page==2` |
| `key@/path` | File upload (multipart) | `avatar@/tmp/photo.png` |
| `key@/path;type=mime` | File upload with explicit MIME | `blob@/tmp/data.bin;type=application/octet-stream` |
| `key=@/path` | String field read from file | `payload=@/tmp/body.txt` |
| `key:=@/path` | JSON field read from file | `config:=@/tmp/opts.json` |

Operator precedence (strict, evaluated left to right): `:=` and `:=@` → `==` → `:` → `=`
and `=@` → `@`.

**JSON body** is the default when data items are present. Pass `--form` for
`application/x-www-form-urlencoded` or `--multipart` for `multipart/form-data`.

---

## Output control

```bash
# Print only response headers
http --print=h GET https://httpbin.org/get

# Print request + response headers (no body)
http --print=Hh GET https://httpbin.org/get

# Print everything
http --verbose GET https://httpbin.org/get

# Disable all formatting and colour (good for piping)
http --pretty=none GET https://httpbin.org/get | jq .

# Change syntax theme
http --style=dracula GET https://httpbin.org/json
```

**`--print` flags** — combine freely:

| Flag | Meaning |
|---|---|
| `H` | Request headers |
| `B` | Request body |
| `h` | Response headers |
| `b` | Response body |

Default is `hb` (response headers + body). `--verbose` is shorthand for `HBhb`.

**`--pretty` modes:** `all` (default when TTY), `colors`, `format`, `none`.

**`--style` themes:** `monokai` (default), `solarized`, `dracula`, `autumn`.

**Quick summary line**

```bash
# Append compact status/time/size summary
http --summary GET https://httpbin.org/get
```

---

## Authentication

```bash
# HTTP Basic
http --auth user:pass https://api.example.com/resource

# Bearer token
http --auth-type bearer --auth "$TOKEN" https://api.example.com/resource

# HTTP Digest (RFC 7616 — MD5 and SHA-256 supported, automatic 401 retry)
http --auth-type digest --auth user:pass https://api.example.com/resource
```

The `--auth-type` flag defaults to `basic`. Credentials passed via `--auth` are masked
(`user:****`) in `--verbose` output.

---

## Sessions

Named sessions persist headers, cookies, and auth credentials between requests. Session
files live in `~/.config/zapreq/sessions/{hostname}/{name}.json`.

```bash
# Log in — session is created and cookies are saved
http --session prod POST https://api.example.com/login username=faizan password=secret

# Subsequent requests reuse the saved session
http --session prod GET https://api.example.com/me
http --session prod GET https://api.example.com/orders

# Load a session but do not update it after the response
http --session-read-only prod GET https://api.example.com/me
```

Session file format:

```json
{
  "headers":  { "X-Client": "zapreq" },
  "auth":     { "type": "basic", "username": "faizan", "password": "secret" },
  "cookies":  [{ "name": "sid", "value": "abc", "domain": "api.example.com", "path": "/" }],
  "meta":     { "created": "2026-02-01T10:00:00Z", "last_used": "2026-05-01T14:22:00Z" }
}
```

---

## Environment profiles

Profiles let you switch base URL, headers, and variable values with a single flag.
Profile files live in `~/.config/zapreq/envs/{name}.json`.

```json
{
  "base_url":  "https://api.example.com",
  "headers":   { "X-API-Version": "2" },
  "variables": { "USER_ID": "42", "TOKEN": "prod-secret" }
}
```

```bash
# Use a named profile
http --env-profile prod GET /users/{USER_ID}

# Combine with a collection
http run get-user --env-profile staging
```

Variable tokens `{KEY}` are substituted in the URL and in all request item values before
the request is built.

---

## Request collections

Collections let you save a request by name and replay it later, optionally with a
different environment profile.

```bash
# Save a request
http save login -- POST https://api.example.com/login username=faizan password={PASSWORD}

# List saved requests
http list

# Run a saved request
http run login

# Run with a profile (profile variables fill {PASSWORD})
http run login --env-profile prod

# Delete a saved request
http delete login
```

Collection files live in `~/.config/zapreq/collections/{alias}.json`.

Structured workspaces (v2):

```bash
# Create/list workspaces
http collections new api
http collections list

# Save/list/run workspace requests
http requests save api get-users -- GET https://api.example.com/users
http requests list api
http requests run api get-users

# Migrate legacy aliases from ~/.config/zapreq/collections/*.json
http collections migrate --workspace legacy
```

Import/export:

```bash
http collections export api ./api-workspace.json --format zapreq
http collections export api ./api-postman.json --format postman
http collections export api ./api-openapi.json --format openapi
http collections import api2 ./api-workspace.json
```

---

## Terminal workspace

Open an interactive terminal workspace for saved requests:

```bash
http tui
```

The workspace provides:
- multi-pane layout (workspaces, requests, response viewer)
- keyboard navigation (`←/→`, `↑/↓`)
- response tabs (`Tab`) for body, headers, meta, and raw
- request filtering (`/` to type filter, `Enter` to apply, `Ctrl+u` to clear)
- inline execution (`Enter`) with response rendered in-place
- environment profile switching (`e`)

---

## API testing

Assert API responses directly from the terminal:

```bash
http test --expect-status 200 --expect-header content-type~json -- GET https://httpbin.org/json
```

JSON assertions use `path=value` syntax:

```bash
http test --expect-json slideshow.title=\"Sample Slide Show\" -- GET https://httpbin.org/json
```

Machine-readable reports:

```bash
http test --report json --expect-status 200 -- GET https://httpbin.org/get
```

Exit codes:
- `0` all assertions passed
- `1` one or more assertions failed
- `2` CLI/runtime error

---

## Secrets

Store and retrieve local secrets:

```bash
http secrets set API_TOKEN super-secret-token
http secrets list
http secrets get API_TOKEN
http secrets get API_TOKEN --reveal
```

Secrets are stored in `~/.config/zapreq/secrets.json`.

---

## AI assistant

Set an OpenAI-compatible API key:

```bash
export ZAPREQ_AI_KEY=sk-...
```

Describe your request in plain English:

```bash
http ai "POST to https://api.example.com/users with name Faizan and role admin"
```

ZapReq prints a generated command first. By default it does not send the request.

```bash
# Generate only (default)
http ai "GET https://api.example.com/me with bearer auth"

# Generate and execute
http ai "GET https://api.example.com/me with bearer auth" --send

# Save generated request
http ai "POST login request to https://api.example.com/login" --save login

# Show generation breakdown
http ai "create users request" --explain
```

---

## Response diffing

Compare two API endpoints key by key. ZapReq flattens both JSON responses into
dot-notation paths and shows what changed.

```bash
http diff https://api.example.com/v1/user/42 https://api.example.com/v2/user/42
```

Output:

```
A: GET https://api.example.com/v1/user/42  →  200 OK
B: GET https://api.example.com/v2/user/42  →  200 OK

  user.id          42
  user.name        "faizan"
- user.role        "admin"          (only in A)
+ user.role        "viewer"         (only in B)
~ user.updated_at  "2025-01-01" → "2026-05-01"
```

Green lines are additions, red are removals, yellow are value changes.

---

## Download mode

```bash
# Download to current directory (filename from Content-Disposition or URL)
http --download https://example.com/archive.zip

# Save to a specific path
http --download --output ~/Downloads/archive.zip https://example.com/archive.zip

# Resume a partial download
http --download --continue --output archive.zip https://example.com/archive.zip
```

A progress bar is shown during the download:

```
[████████████░░░░░░░░] 4.2 MB / 10.0 MB  ·  1.2 MB/s  ·  ETA 5s
✔ Downloaded: archive.zip  (10.0 MB in 8.3s  ·  avg 1.2 MB/s)
```

---

## Configuration

Config file: `~/.config/zapreq/config.json`

```json
{
  "default_options": ["--style=monokai"],
  "default_scheme":  "https",
  "plugins_dir":     "~/.config/zapreq/plugins",
  "output_theme":    "monokai",
  "pretty":          "all",
  "verify":          true
}
```

| Key | Type | Default | Description |
|---|---|---|---|
| `default_options` | `string[]` | `[]` | Flags prepended before every invocation |
| `default_scheme` | `string` | `https` | Scheme used when URL has no scheme |
| `plugins_dir` | `string` | `~/.config/zapreq/plugins` | Directory scanned for plugin manifests |
| `output_theme` | `string` | `monokai` | Default syntax theme |
| `pretty` | `string` | `all` | Default pretty mode |
| `verify` | `bool` | `true` | TLS certificate verification |

Precedence order (highest to lowest):

```
Explicit CLI flags
  ↓
ZAPREQ_DEFAULT_OPTIONS environment variable
  ↓
config.json default_options
  ↓
Built-in defaults
```

---

## Plugins

Built-in plugins (`basic`, `bearer`, `digest`) are always available. Third-party plugins
are discovered from `plugins_dir` via `.toml` manifest files.

```bash
# List all registered plugins
http plugins list

# Validate plugin manifests and executable paths
http plugins validate

# Install instructions for a community plugin
http plugins install zapreq-plugin-aws

# Run a plugin executable (if manifest defines executable)
http plugins run my-plugin -- --help
```

Manifest format (`~/.config/zapreq/plugins/my-plugin.toml`):

```toml
[plugin]
name        = "my-auth"
version     = "1.0.0"
description = "Custom HMAC authentication"
auth_types  = ["hmac"]
executable  = "./my-auth-plugin"
```

See the [plugin authoring guide](https://github.com/MFAIZAN20/zapreq/wiki/plugins) for
how to build and distribute a zapreq plugin.

---

## Comparison with HTTPie

| Feature | HTTPie | zapreq |
|---|---|---|
| JSON / form / multipart | ✅ | ✅ |
| Sessions | ✅ | ✅ |
| Basic + Bearer auth | ✅ | ✅ |
| Digest auth (RFC 7616) | ✅ | ✅ |
| Syntax-highlighted output | ✅ | ✅ |
| Environment profiles | ❌ | ✅ |
| Request collections | ❌ | ✅ |
| Interactive terminal workspace (`http tui`) | ❌ | ✅ |
| API test assertions (`http test`) | ❌ | ✅ |
| Local secrets store (`http secrets`) | ❌ | ✅ |
| AI request assistant | ❌ | ✅ |
| Response diffing | ❌ | ✅ |
| Resume downloads | ❌ | ✅ |
| Native binary (no Python) | ❌ | ✅ |
| Binary size | ~15 MB | ~3 MB |
| Startup time | ~200 ms | ~5 ms |

---

## Contributing

```bash
# Clone
git clone https://github.com/MFAIZAN20/zapreq
cd zapreq

# Run tests
cargo test --all

# Lint
cargo clippy --all-targets --all-features -- -D warnings

# Format
cargo fmt --all

# Release build
cargo build --release
```

All pull requests must pass the CI matrix (Ubuntu, macOS, Windows × stable, beta) before
merge. Please open an issue before starting work on a large feature.

---

## License

Licensed under either of the following, at your option:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

This is the standard dual license used across the Rust ecosystem (Rust itself, Cargo,
tokio, serde, clap, reqwest). You may choose whichever license suits your project.

Copyright © 2026 Muhammad Faizan

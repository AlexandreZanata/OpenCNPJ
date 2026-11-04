# OpenCNPJ Admin (desktop)

Native administration application for an OpenCNPJ server, built with
[eframe/egui](https://github.com/emilk/egui) (OpenGL through `glow`). It talks to the admin
listener (`serve --admin-bind`) over HTTP, using the API described in
[`docs/ACCOUNTS.md`](../../docs/ACCOUNTS.md). Binary name: `opencnpj-admin`.

Runs on Linux (X11 and Wayland) and Windows. Every HTTP call runs on a background thread; the
window never blocks on the network.

## Screens

| Screen | What it does | Endpoints |
|---|---|---|
| Login | Server admin URL (default `http://127.0.0.1:8081`) and login key | `POST /api/v1/auth/login`, `GET /admin/api/whoami`, `POST /api/v1/auth/logout` |
| Dashboard | KPIs, requests/s and latency of the last 5 minutes, per-route latency, caches; refreshes every 2 s | `GET /admin/api/overview`, `GET /admin/api/metrics?seconds=300`, `GET /admin/api/response-cache` |
| Users | Search, filter (status, role, plan), sort, page; create; detail with profile, overrides editor, notes, suspend / ban / reactivate / rotate key / revoke sessions / reset usage / delete, 30-day usage chart, keys, sessions, recent searches | `GET/POST /admin/api/users`, `GET/PATCH/DELETE /admin/api/users/{id}`, `POST /admin/api/users/{id}/{suspend,ban,reactivate,rotate-key,revoke-sessions,reset-usage}`, `GET /admin/api/roles` |
| Plans | Table and create / edit / delete of every plan field | `GET/POST /admin/api/plans`, `PATCH/DELETE /admin/api/plans/{id}` |
| Usage | Daily requests, active users and cache-hit rate, requests by kind, top users today, totals | `GET /admin/api/usage?days=N` |
| Searches | Search log with a user filter and "load more" | `GET /admin/api/searches?user_id=&limit=&before=` |
| Audit | Audit log with actor / target filters and "load more" | `GET /admin/api/audit?actor_id=&target_id=&limit=&before=` |
| Operations | Snapshots (activate, reload), jobs (start, stop, live log), updates (status, check now), postings and response caches, server settings | `/admin/api/snapshots[/activate]`, `/admin/api/reload`, `/admin/api/jobs[/{id}[/log]]`, `/admin/api/updates[/check]`, `/admin/api/cache`, `/admin/api/response-cache`, `/admin/api/settings` |

Screens and actions the signed-in role lacks permissions for are disabled (with a tooltip
naming the permission). API errors appear as notifications and in the status bar; a `401`
returns to the login screen.

## Credentials and stored data

The login key and the session token are kept in memory only. The application writes one file,
`config.json`, with the server URL and (if "Remember my username" is ticked) the username:

| Platform | Location |
|---|---|
| Linux | `$XDG_CONFIG_HOME/opencnpj-admin/` or `~/.config/opencnpj-admin/` |
| Windows | `%APPDATA%\opencnpj-admin\` |
| macOS | `~/Library/Application Support/opencnpj-admin/` |

Signing out revokes the session on the server. Keys created or rotated from the application are
shown once, with a copy button.

## Build

Requires Rust 1.88 or newer. No system development packages are needed on Linux: X11, Wayland
and OpenGL libraries are loaded at run time.

```bash
cargo build --release -p opencnpj-desktop      # target/release/opencnpj-admin(.exe)
cargo run -p opencnpj-desktop                  # debug build
cargo test -p opencnpj-desktop
```

At run time Linux needs an OpenGL 2+ (or OpenGL ES 2+) driver, `libxkbcommon`, and
`libwayland-client` or the X11 libraries; desktop distributions ship all of these.

The server must be started with an admin listener and accounts, for example:

```bash
opencnpj serve --data-dir DATA --admin-bind 127.0.0.1:8081
opencnpj accounts --data-dir DATA create --username admin --email admin@example.com --role admin
```

The `create` command prints the login key once; use it on the login screen. Staff roles are
`admin`, `moderator` and `analyst`; `user` accounts cannot sign in to the admin API.

## Install on Linux

From a release build, for the current user:

```bash
cargo build --release -p opencnpj-desktop
crates/opencnpj-desktop/install-linux.sh
```

The script installs (and strips) the binary to `~/.local/bin/opencnpj-admin`, the icon to
`~/.local/share/icons/hicolor/scalable/apps/opencnpj-admin.svg` and the desktop entry to
`~/.local/share/applications/opencnpj-admin.desktop` (with an absolute `Exec` path), then
refreshes the desktop database. `PREFIX=/some/dir` installs elsewhere, `BIN=path` picks another
binary, and `--uninstall` removes the three files. By hand:

```bash
install -Dm755 target/release/opencnpj-admin ~/.local/bin/opencnpj-admin
install -Dm644 crates/opencnpj-desktop/assets/opencnpj-admin.svg \
    ~/.local/share/icons/hicolor/scalable/apps/opencnpj-admin.svg
install -Dm644 crates/opencnpj-desktop/assets/opencnpj-admin.desktop \
    ~/.local/share/applications/opencnpj-admin.desktop
```

## Install on Windows

```powershell
cargo build --release -p opencnpj-desktop
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\OpenCNPJ Admin" | Out-Null
Copy-Item target\release\opencnpj-admin.exe "$env:LOCALAPPDATA\Programs\OpenCNPJ Admin\"
# Start menu shortcut
$s = (New-Object -ComObject WScript.Shell).CreateShortcut("$env:APPDATA\Microsoft\Windows\Start Menu\Programs\OpenCNPJ Admin.lnk")
$s.TargetPath = "$env:LOCALAPPDATA\Programs\OpenCNPJ Admin\opencnpj-admin.exe"
$s.Save()
```

Release builds on Windows are GUI-subsystem executables (no console window); debug builds keep
the console for diagnostics. The window icon is drawn by the application itself; the `.exe`
file has no embedded icon resource.

## Layout

| Path | Purpose |
|---|---|
| `src/main.rs` | Entry point (Windows GUI subsystem in release builds) |
| `src/lib.rs` | Window options and `run()` |
| `src/app.rs` | Shell: login, navigation, status bar, notifications, theme |
| `src/api.rs` | Blocking HTTP client (`ureq`, rustls) and `Task`, the background-call helper |
| `src/models.rs` | JSON models (`serde(default)`, tolerant of `null` and missing fields) |
| `src/state.rs` | Session, permissions, notifications, cross-screen signals |
| `src/config.rs` | Preferences file location and load/save |
| `src/screens/*.rs` | One module per screen |
| `src/widgets.rs`, `src/theme.rs`, `src/format.rs`, `src/icon.rs` | Widgets and charts, styles, formatting, procedural icon |
| `assets/` | Icon (SVG) and freedesktop entry |
| `tests/client.rs` | Client tests against an in-process HTTP server |

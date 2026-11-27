# AGENTS.md

Windows desktop status panel: Rust + eframe/egui on **wgpu** (not glow), cross-built
from Linux and handed over as a zip. No README, no CI; tests live in each module.

## Commands

```sh
cargo test                                   # all unit tests, inline #[cfg(test)] per module
cargo test config::                          # one module's tests
cargo run -- --check                         # fetch every source, print results, no window
cargo run -- --mem                           # + append status-panel-mem.log every 5 s
```

Release build (the deliverable is Windows, never the Linux binary):

```sh
cargo build --release --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/status-panel.exe dist/
python3 -m zipfile -c status-panel-windows.zip dist/status-panel.exe dist/status-panel.conf
```

- The Windows target needs mingw-w64: `build.rs` hardcodes
  `x86_64-w64-mingw32-windres` for the exe icon, so a missing compiler fails the
  release build, not `cargo test`/`cargo build` on Linux.
- Visual checks on this Linux box (no display): `Xvfb :99`, run with
  `DISPLAY=:99 WGPU_BACKEND=vulkan`, resize via `xdotool search --name "Status Panel"`,
  capture with `scrot`. Screenshots are the only way to verify egui layout changes.

## Deployment

The deliverable is the zip; nothing runs on this Linux box.

1. Copy `status-panel-windows.zip` to the Windows machine, extract `status-panel.exe` and
   `status-panel.conf` into one folder (the exe reads the conf from its own directory, then cwd).
2. Put the machine's real tokens in that `status-panel.conf`; `status-panel.conf.example` documents
   every key and must stay in sync when a key is added.
3. Autostart: a shortcut to the exe in `shell:startup`, or a scheduled task at logon. The exe is a
   GUI app — no console, no service, no port.
4. Verify: the header shows the gateway badge and `latest v2 <tag> released <date>`; a red header
   warning means a font name in the conf is not installed.

Upgrade = close the panel (or use the tray icon), replace the exe, keep the conf. `zoom=` is written
back into the conf at runtime, so the user's zoom survives the swap.

## Config and secrets

- `Config::load()` reads `status-panel.conf` from the cwd, then beside the exe.
  Plain `key=value`, unknown keys ignored, `block.<id>.visible|minimized` is a group.
- `status-panel.conf`, `dist/`, `status-panel-windows.zip`, `target/` are gitignored.
  `dist/status-panel.conf` holds **real tokens** — never commit it or paste its values.
  Only `status-panel.conf.example` is tracked; keep it in sync when adding keys.
- `Config::save_zoom()` rewrites the `zoom=` line of the loaded conf file; the UI writes
  the user's zoom back there, so a running instance mutates conf at runtime.

## How data flows

`main.rs` loads config → spawns `worker` → `mpsc` channel of `Update` → `App` merges into
`Snapshot`. The worker sends **one `Update` per source** (gateway, stats, tasks), each
immediately, and runs the GitHub release check *after* them. Do not batch sources into a
single message: that made every card wait on the slowest request and is the "loading..."
bug that was fixed. `Snapshot::apply` keeps fields not present in an update.

Sources and their separate credentials:

- MCP gateway `GET gateway_url` (`.../status`) with `gateway_token`.
- OpenCode token stats `opencode_url` with basic auth `opencode_user/password`.
- Ongoing tasks: JSON-RPC `POST {gateway host}/mcp` with **`mcp_token`** (the status token
  gets 401 there), `tools/call` → `call_tool` → `server: "obsitrace"`, `tool: "query_tasks"`.
  URL is derived by `Config::mcp_url()`; don't add a second URL key.
- Release check: first non-draft, non-prerelease `v2.*` tag wins; header renders
  `latest v2 <tag> released <YYYY-MM-DD>`.

## UI structure

- Cards are content-sized: card widths are measured each frame into `App::widths` and
  `layout::rows` packs them side by side until the row is full. Anything that changes card
  content changes wrapping — `layout.rs` has the row tests.
- `tasks_table` sizes the TASK column to the longest title, capped at
  `available_width - 340` (min 160) so PROJECT + UPDATED stay visible. Re-check for a
  horizontal scrollbar when changing those columns.
- Block ids come from config; a new id needs arms in `App::render_block` and
  `ui::block_title`, otherwise the UI shows "no renderer for block".
- Minimize is drawn top-right in `render_card`; minimized blocks become chips placed to the
  right of the last row when the leftover width allows, otherwise on their own row.
- Windows-only: tray icon, close-to-tray, taskbar flag (`cfg(windows)` in `app.rs`/`main.rs`).
  Off Windows the taskbar button is forced on.

## Fonts and assets

- `assets/MapleMono-CN-Regular.subset.ttf` (5.1 MB) is compiled in with `include_bytes!`;
  `theme::DEFAULT_FONT` is `Maple Mono CN`. `is_bundled` treats empty, `maple*` and `noto*`
  (the old name) as the bundled font; any other name is a system font and shows a red header
  warning if missing.
- Windows font collections map by name + face index (`windows_face`); YaHei UI is
  `msyh.ttc` face 1. A plain `.ttf` name would silently load the wrong face.
- To replace the bundled font: subset to GB2312 + Latin + kana + punctuation + the glyphs the
  UI itself prints (`■` U+25A0, `−` U+2212) with `pyftsubset` (fonttools), keep the OFL
  license next to it, then verify `cargo test theme::` still passes its size/format asserts.

## Gotchas

- `build.rs` pulls `src/icon_art.rs` in via `mod art { include!(...) }`, so that file must
  stay dependency-free (no `crate::`, no egui) — it is compiled twice, also for the tray/window
  icon in `src/icon.rs`.
- wgpu is deliberate: DX12-only on Windows and `MemoryHints::MemoryUsage` exist to cut memory;
  don't "simplify" them away or swap in the glow backend.
- Edition 2024; formatting follows rustfmt defaults.
- Push as `Jnchk <SarshesJ@hotmail.com>` with
  `GIT_SSH_COMMAND='ssh -i ~/.ssh/github -o IdentitiesOnly=yes' git push origin main`
  (`origin` is `git@github.com:Jnchk/mcp-status-panel.git`).

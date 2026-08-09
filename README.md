# shinra-engine

The Rust + wgpu game engine plus its tooling: render core (`engine`), serde
scene format (`scene`), cdylib FFI types (`abi`), and two ways to run games —
`runner` (terminal player for cdylib games) and the `frontend/` IDEs
(`tui` ratatui IDE, `gui` Tauri + Svelte scaffold).

Game **data** lives in a separate sibling project (e.g.
[`shinra-examples`](../shinra-examples/)) — a folder per game under
`assets/games/<name>/` holding `scene.ron`. The TUI IDE loads scene files,
renders in-terminal, and **`n`** in the IDE cycles to the next game.

## Two coexisting game models

This repo currently supports two ways of describing a game; they're being
unified, not maintained in parallel forever.

| Model | Lives in | Loader | Status |
|---|---|---|---|
| **scene-based** (data) | `<project>/assets/games/<name>/scene.ron` | `frontend/tui` (working), `frontend/gui` (scaffold) | Current direction. |
| **cdylib** (code) | `target/debug/libgame*.so`, built from a Rust crate via the `.hom` DSL → `homunc` → rustc | `runner` | Legacy. The build infra (`hom_hecs` runtime, `homunc` integration, build.rs templates) still needs to be relocated into this repo; see "Roadmap". |

A scene-based game is a single `scene.ron` (a `scene::Scene`): nodes with
transforms, optional `mesh:` OBJ refs (`assets/obj/...`), optional `sprite:`
(a quad UV-cut from a sheet PNG: `sheet`, `grid`, `cell`, `size`), optional
`tilemap:` (+ `.tres.ron` tileset), behavior `components:`
(`PlayerControlled`, `ScrollX`, `Obstacle` — interpreted by the IDE's
running mode, plus `Dialogue` for Space-advanced visual-novel text), and an
optional embedded `camera:` — the engine falls back to
a default perspective camera when it's absent.

We call the architecture **gametok**: TikTok-style swipe between games. `n`
is consumed by the loader, never seen by the game.

## Prerequisites (Ubuntu / Debian, only if building natively)

```bash
# Rust toolchain — current stable (1.88+ required by wgpu 27)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
. "$HOME/.cargo/env"

# C/C++ toolchain
sudo apt install -y build-essential cmake nasm pkg-config

# Vulkan runtime for headless wgpu rendering (TUI IDE, tests)
sudo apt install -y mesa-vulkan-drivers libvulkan1 vulkan-tools
```

## Run natively

```bash
cargo build                        # builds engine, scene, abi, runner, tui
cargo run -p tui                   # TUI IDE (ratatui, runs in terminal)
cargo run -p runner                # terminal mode; cycles libgame*.so in target/debug
```

Run from inside a game project directory (e.g. `shinra-examples/`) so the IDE
can find `assets/games/`.

## TUI IDE

The TUI IDE (`frontend/tui` crate, binary `tui`) is a single Rust binary that
replaces the former editor-server + VS Code extension stack. One process, one
language, zero network. Works over SSH, in tmux, headless.

```bash
cd ../shinra-examples              # or any game project with assets/games/
cargo run -p tui --manifest-path ../shinra-engine/Cargo.toml
```

### Layout

```
+-----------------------------------------------------------------------+
|  File  Edit  View  Run                              game 1/2: bunny   |
+-----------------------+-------------------------------+---------------+
| Hierarchy             | Viewport                      | Inspector     |
|                       |                               |               |
| > Main Camera         |     [ Player ]                | Name: Player  |
| > Player              |                               | Position:     |
|                       |         [ Enemy ]             |   X: 10.0     |
|                       |                               |   Y: 5.0      |
+-----------------------+-------------------------------+---------------+
| Project Browser       | Terminal                                      |
| assets/               | $ bash (embedded PTY)                         |
+-----------------------+-----------------------------------------------+
```

Five panels: Hierarchy (scene tree), Viewport (wgpu render), Inspector
(selected node transform), Project Browser (file tree), and Terminal — a real
embedded bash shell (portable-pty + vt100). Clicking a panel with the mouse
focuses it.

### Viewport render modes

The viewport renders the wgpu frame as Unicode text-art by default, so the
scene reads like text instead of solid color blocks. `m` cycles the mode:

| Mode | Glyphs |
|---|---|
| `Mixed` (default) | dim cells as braille dots `⣸⣿⣆`, bright cells as quadrant blocks `▛█▜` — ink density doubles as shading |
| `Quadrant` | 2×2 block glyphs ` ▘▝▀▖▌▞▛▗▚▐▜▄▙▟█` (16 patterns) |
| `Braille` | 2×4 dot glyphs U+2800–U+28FF (256 patterns, 8× cell resolution) |
| `Image` | ratatui-image protocol (kitty / sixel / halfblock fallback) |

Text modes run on the GPU: a compute pass (`engine/src/textart.wgsl`,
wrapped by `engine/src/textart_gpu.rs`) renders at 2×4 subpixels per cell,
thresholds each subpixel by linear luminance, and packs one
`u32` per cell (`braille bits << 24 | sRGB r/g/b`). The CPU readback is one
int per cell, decoded to glyphs by `textart::packed_to_cells` (a CPU
reference implementation of the pass lives in `engine/src/textart.rs` for
tests). The camera aspect is corrected for ~1:2 terminal cells so geometry
keeps its proportions.

### Default keybindings

Configurable bindings (`ide.ron`):

| Key     | Action               |
|---------|----------------------|
| Ctrl+H  | focus Hierarchy      |
| Ctrl+V  | focus Viewport       |
| Ctrl+I  | focus Inspector      |
| Ctrl+F  | focus Project browser|
| Ctrl+T  | focus Terminal       |
| m       | cycle viewport render mode |
| r       | toggle running (play) mode |
| q       | quit                 |

Built-in keys (not configurable):

| Key     | Action                                        |
|---------|-----------------------------------------------|
| n       | cycle to next game                            |
| Tab     | cycle focused panel                           |
| Ctrl+S  | save current game's `scene.ron` back to disk  |
| Esc     | quit (unless editing in the Inspector)        |

Per-panel keys: Hierarchy / Project use ↑/↓ to move and Enter to
expand/collapse; in the Inspector, `e` enters edit mode, then Tab cycles
fields, `+`/`-` adjust the value by 0.1 (applied to the scene live), and Esc
exits edit mode.

### Running mode

`r` plays the current game on a cloned scene, like the real game runner —
the editor scene is untouched. While running:

| Key   | Action                                  |
|-------|-----------------------------------------|
| space | next dialogue line, or jump in action games |
| n     | swipe to the next game (restarts the run) |
| esc / r | stop and return to the editor         |

Behavior comes from `components:` in `scene.ron`, ticked each frame with
real dt (`frontend/tui/src/core/run.rs`): `PlayerControlled` gets gravity +
the space jump impulse; `ScrollX(speed, wrap_at, reset_to)` auto-scrolls a
node along X with wrap-around; colliding with an `Obstacle` node resets the
run. See `shinra-examples/assets/games/game3` — a Chrome-offline-style dino
run built from the `assets/images/2x2_grid.png` sprite sheet (dino / tree /
cloud / bird).

`Dialogue(lines: [...])` turns the same overlay system into a simple galgame
text box: each line has a `speaker` and `text`, and Space advances to the next
line. See `shinra-examples/assets/games/game4` for a complete example.

Game4 uses the reusable text-box overlay system
(`frontend/tui/src/core/overlay.rs` + `tui/overlay.rs`). The dialogue overlay
is only created for scenes containing `Dialogue`; games 1–3 do not show it.

Note: configurable bindings are resolved first, even while the Terminal panel
is focused — with the default config you cannot type `q` into the embedded
shell. Stderr (Mesa/EGL driver noise) is redirected to
`/tmp/shinra-ide-stderr.log`.

### Configuration

Keybindings are configurable via `ide.ron` in the working directory:

```ron
(
    mode: Tui,
    keybindings: {
        "ctrl+h": "focus_hierarchy",
        "ctrl+v": "focus_viewport",
        "ctrl+i": "focus_inspector",
        "ctrl+f": "focus_project",
        "ctrl+t": "focus_terminal",
        "m": "toggle_viewport_mode",
        "r": "toggle_run",
        "q": "quit",
    },
    viewport_mode: Mixed,   // Mixed | Quadrant | Braille | Image
)
```

`mode: Gui` is accepted but currently a no-op (the binary exits immediately).

## GUI (Tauri + Svelte scaffold)

`frontend/gui` is a Svelte 5 + Vite mirror of the TUI's panel layout
(Hierarchy, Project, Viewport, Inspector, Terminal, Console) intended to
become a native Tauri 2 app. Today it's a browser-only scaffold with mock
state — no engine render in the viewport yet. The `src-tauri` crate (with
`load_scene` / `save_scene` commands) is **excluded from the workspace**
(needs `npm install` + network) and does not currently compile.

```bash
cd frontend/gui
npm install
npm run dev                        # vite dev server on :1420
# or, containerized:
docker compose up gui              # from the repo root
```

## Workspace layout

```
shinra-engine/
├── engine/             shinra-engine — wgpu device, mesh + sprite render
│                       pipelines, scene loading, readback/snapshot, text-art
│                       compute pass, presenters, Keymap, EngineBackend trait
├── abi/                gametok-abi — #[repr(C)] InputFrame, Drawable (cdylib FFI)
├── scene/              serde scene format (scene.ron: nodes, tilemaps, camera)
├── runner/             terminal binary; dlopen + render loop + n-swipe
├── frontend/
│   ├── tui/            TUI IDE — ratatui 5-panel layout, wgpu offscreen render,
│   │                   embedded PTY terminal (bin name: tui)
│   ├── gui/            Tauri 2 + Svelte 5 IDE scaffold + Playwright e2e
│   └── tests/snapshots/  shared TUI/GUI snapshot baselines
├── docker-compose.yml  gui dev server, e2e + rust-test profiles
├── Dockerfile          dev image (used by the rust-test service; its default
│                       CMD still targets the removed editor-server)
└── Dockerfile.release  slim multi-stage image (same stale editor-server CMD)
```

## The gametok cdylib FFI (legacy)

A cdylib game must export five C symbols. The runner dlopens it, calls
`tick(dt, input)` per frame, and reads `drawables_ptr` / `drawables_len`.

```rust
extern "C" fn meshes_count() -> u32;
extern "C" fn meshes_path(i: u32, out: *mut u8, cap: u32) -> u32;
extern "C" fn tick(dt: f32, input: *const InputFrame);
extern "C" fn drawables_ptr() -> *const Drawable;
extern "C" fn drawables_len() -> u32;
```

`Drawable { mesh_id, _pad, model: [f32; 16] }` — column-major mat4. The
runner copies into a transient `Scene`, calls `engine.render(&scene)`, and
presents via the terminal presenter (viuer). Input: `w/a/s/d` move, arrows
rotate, `j/k` scale, `n` next game, `q`/Esc quit.

The `.hom` DSL → `homunc` → rustc cdylib pipeline that produced these
formerly shipped in `shinra-examples/games/`. The build infrastructure
(templates, `hom_hecs` runtime, `homunc` invocation) still needs to be
relocated into this repo; see "Roadmap".

## Tests

```bash
cargo test --workspace             # unit + integration tests, all crates
ls target/debug/smoke/             # cube.png teapot.png bunny.png — render smoke outputs
```

- `engine/tests/render_smoke.rs` — renders cube/teapot/bunny to PNG (skips
  meshes whose assets are missing).
- `engine/tests/snapshot_test.rs` + `frontend/tui/tests/` — PNG and TUI-text
  snapshot baselines under `frontend/tests/snapshots/`; refresh with
  `UPDATE_SNAPSHOTS=1 cargo test`.
- `engine/tests/frontend_parity_test.rs` — proves TUI and GUI share one
  deterministic `EngineBackend` code path.
- GUI e2e: `cd frontend/gui && npm test` (Playwright against the vite dev
  server), or containerized: `docker compose --profile test up e2e`.
- Workspace tests in Docker: `docker compose --profile test run rust-test`.

## Roadmap

1. **Runner reads scene-based games.** Today `runner` only knows about
   `target/debug/libgame*.so`. Teach it to also (or instead) cycle
   `assets/games/*/scene.ron` so the IDE and runner share one game model.
2. **Move cdylib build infra into this repo.** `homunc`, `hom_hecs/`, and
   the per-game `build.rs` template currently expect to live next to game
   source. Lift them into this repo and let the IDE "scaffold game" command
   stamp out a new game folder against this repo's templates.
3. **Finish the GUI.** Wire the Svelte scaffold to the engine (viewport
   render, real scene load/save through the Tauri commands) and bring
   `src-tauri` back into the workspace.

## Status

POC complete: the TUI IDE loads scene-based games (meshes, sprites,
tilemaps), renders them in-terminal as Unicode text-art, edits node
transforms through the Inspector, saves back to `scene.ron` with Ctrl+S, and
plays them in running mode (`r`) — game3 is a playable dino-run mini game.
The GUI is a tested (Playwright) but engine-less scaffold. The native runner
still loads cdylib `.so` files only.

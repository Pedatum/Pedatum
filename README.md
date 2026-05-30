# shinra-engine-core

The Rust + wgpu game engine plus its tooling: render core, scene types,
front-ends (`runner` for terminal, `ide` for TUI, `editor` for native GUI),
and the build infrastructure for games.

Game **data** lives in a separate project (e.g.
[`shinra-examples`](../shinra-examples/)) — a folder per game holding
`scene.ron` + `tscn.ron`. The TUI IDE loads scene files, renders in-terminal,
and **`n`** in the viewport cycles to the next.

## Two coexisting game models

This repo currently supports two ways of describing a game; they're being
unified, not maintained in parallel forever.

| Model | Lives in | Loader | Status |
|---|---|---|---|
| **scene-based** (data) | `<project>/assets/games/<name>/{scene.ron,tscn.ron}` | `ide` / `editor` (working) | Current direction. |
| **cdylib** (code) | `<project>/games/<name>/` Rust crate, compiled to `libgame*.so` via `.hom` DSL → `homunc` → rustc | `runner` (`target/debug/libgame*.so`) | Legacy. The build infra (`hom_hecs` runtime, `homunc` integration, build.rs templates) belongs in this repo so projects don't carry it; see "Roadmap" below. |

We call the architecture **gametok**: TikTok-style swipe between games. `n`
is consumed by the loader, never seen by the game.

## Prerequisites (Ubuntu / Debian, only if building natively)

For native builds:

```bash
# Rust toolchain — current stable (1.88+ required by wgpu 27)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
. "$HOME/.cargo/env"

# C/C++ toolchain
sudo apt install -y build-essential cmake nasm pkg-config

# Vulkan runtime for headless wgpu rendering (ide / editor)
sudo apt install -y mesa-vulkan-drivers libvulkan1 vulkan-tools
```

## Run natively

```bash
cargo build                        # builds engine, scene, runner, editor, ide
cargo run -p ide                   # TUI IDE (ratatui, runs in terminal)
cargo run -p editor                # native egui editor
cargo run -p runner                # terminal mode; cycles libgame*.so in target/debug
```

Run from inside a game project directory (e.g. `shinra-examples/`) so the IDE
can find `assets/games/`.

## TUI IDE

The TUI IDE (`ide` crate) is a single Rust binary that replaces the former
editor-server + VS Code extension stack. One process, one language, zero
network. Works over SSH, in tmux, headless.

```bash
cd ../shinra-examples              # or any game project with assets/games/
cargo run -p ide
```

### Layout

```
+-----------------------------------------------------------------------+
|  File  Edit  View  Run                                     [Mem: 12MB]|
+-----------------------+-------------------------------+---------------+
| Hierarchy             | Viewport                      | Inspector     |
|                       |                               |               |
| > Main Camera         |     [ Player ]                | Name: Player  |
| > Player              |                               | Position:     |
|                       |         [ Enemy ]             |   X: 10.0     |
|                       |                               |   Y: 5.0      |
+-----------------------+-------------------------------+---------------+
| Project Browser       | Terminal / Console                            |
| assets/               | [INFO] Engine initialized.                    |
+-----------------------+-----------------------------------------------+
```

Five panels: Hierarchy (scene tree), Viewport (wgpu render via
ratatui-image), Inspector (selected node properties), Project Browser
(file tree), and Terminal/Console (log output).

### Default keybindings

| Key     | Action               |
|---------|----------------------|
| Ctrl+H  | focus Hierarchy      |
| Ctrl+V  | focus Viewport       |
| Ctrl+I  | focus Inspector      |
| Ctrl+F  | focus Project browser|
| Ctrl+T  | focus Terminal       |
| q       | quit                 |
| n       | cycle to next game   |

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
        "q": "quit",
    },
)
```

## Workspace layout

```
shinra-engine-core/
├── engine/         shinra-engine — wgpu device, render pipeline, presenter, Keymap
├── abi/            gametok-abi   — #[repr(C)] InputFrame, Drawable (cdylib FFI)
├── scene/          serde scene + camera types (scene.ron / tscn.ron)
├── runner/         terminal binary; dlopen + render loop + n-swipe
├── editor/         native egui editor (eframe + wgpu)
├── ide/            TUI IDE — ratatui 6-panel layout, wgpu offscreen render
├── Dockerfile      dev image (cargo at container start)
└── Dockerfile.release  slim multi-stage runtime image
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

`Drawable { mesh_id, model: [f32; 16] }` — column-major mat4. The runner
copies into a transient `Scene`, calls `engine.render(&scene)`, and presents.
Each game's `hecs::World` lives in its own `thread_local!`, so swiping
between games is a clean state reset — nothing leaks across `dlclose`.

The `.hom` DSL → `homunc` → rustc cdylib pipeline that produced these
formerly-shipped in `shinra-examples/games/`. It still exists; the build
infrastructure (templates, `hom_hecs` runtime, `homunc` invocation) needs to
be relocated into this repo and exposed via the editor (e.g. as a "scaffold a
new cdylib game" command). See "Roadmap".

## Tests

```bash
cargo test                  # unit + render smoke test
ls target/debug/smoke/      # cube.png teapot.png bunny.png — sanity render outputs
```

## Roadmap

1. **Runner reads scene-based games.** Today `runner` only knows about
   `target/debug/libgame*.so`. Teach it to also (or instead) cycle
   `assets/games/*/scene.ron` so editor and runner share one game model.
2. **Move cdylib build infra into this repo.** `homunc`, `hom_hecs/`, and
   the per-game `build.rs` template currently expect to live next to game
   source. Lift them into `engine-core` and let the editor "scaffold game"
   command stamp out a new game folder against this repo's templates.
3. **Save edits from the viewport.** Arrow-key translations are in-memory
   only; add an explicit save key (or HTTP endpoint) that writes the current
   game's `scene.ron` / `tscn.ron` back to disk.

## Status

POC complete: the TUI IDE loads and renders scene-based games in-terminal
via ratatui-image. The native runner still loads cdylib `.so` files only.

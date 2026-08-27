# Shinra Engine — Design

## Concepts

| | Is |
|---|---|
| **World** | entities and components; the DataPipeline's domain |
| **View** | a GraphicsPipeline that renders as a texture |

A View is an entity. It names the World it renders, which need not be the World
it lives in. Its kind is `View3D`, `View2D` or `ViewText`; its output is a
texture of a declared type and size — `Pixels` or `Cells`.

Every View renders to a texture. There is no direct-to-screen path: the surface
takes a texture like any other consumer.

A View's texture is referenceable by any entity in any World.

```
World A (entities + systems)        World B (entities + systems)
  |                                   ^
  | holds a View entity               | the View names B as its target
  v                                   |
View (3D | 2D | Unicode) -------------+
  |
  | --stages--> texture (Pixels | Cells)
  v
ViewRef, sampled by an entity in any World
```

By convention the root View's entity is named `game_camera`: the main camera,
the largest subject of the game. The IDE holds its own editor camera, separate
from the game camera.

## Terminology

| Term | Is |
|---|---|
| **Pixel** | one colour sample. A `Pixels` frame is a grid of them. |
| **Cell** | one terminal character slot: a character plus a foreground colour, optionally a background. Indivisible, so a `Cells` frame cannot be scaled. Roughly 1:2 (width:height). |
| **`Pixels`** / **`Cells`** | the two frame types. Both are textures; they differ in element type. |
| **Character art** | characters arranged to depict a thing, authored by hand. The `CharArt` component. |
| **Font atlas** | one PNG holding the bitmap of every character, indexed by codepoint. A resource, not a stage. |
| **Atlas** | any single image holding many sub-images addressed by index. A sprite sheet is an atlas; so is a font atlas. |
| **`GlyphSet`** | which characters `ToCells` may emit: `Mixed`, `Quadrant`, `Braille`. |
| **Stage** | a conversion between the two frame types: `ToCells`, `ToPixels`. |

`ToCells` samples 2×4 subpixels per cell and thresholds each by luminance,
giving 8 bits per cell — the braille dot layout. `ToPixels` draws one quad per
cell, its uv addressing that character in the font atlas.

The character set is closed: braille U+2800–U+28FF (256), quadrant blocks (16),
box drawing, printable ASCII. About 400 characters, so the font atlas is baked
rather than rasterised at runtime.

## Layering

The engine provides core capability. A game provides its own behaviour, in
`.hom`, in its own project. The engine contains no game rule and no tuning
constant.

| Engine core | Game (`.hom`) |
|---|---|
| World, entities, component registry | its own component types |
| Views, stages, render graph | its cameras' behaviour |
| transform propagation | gravity, jump, scrolling, collision response |
| asset loading | dialogue flow, win/lose rules |
| raw input events, action-map resolution | its action map and key choices |
| scene serialization, scheduler | its tuning constants |

Three mechanisms make this possible.

### Open component registry

Engine-core components are typed. Game components are open, keyed by type name
and resolved through a registry the game's compiled `.hom` module populates.

```ron
components: {
    "PlayerControlled": ( jump_velocity: 10.5, gravity: -32.0 ),
    "Obstacle": (),
}
```

An unregistered component name is a load error naming the component and the
module expected to provide it. Adding a game behaviour never edits the engine.

### Named input actions

The engine emits raw key events and resolves them against an action map the game
declares. The engine binds no key to any meaning.

```ron
// games/dino-run/input.tres.ron
(
    actions: { "jump": [ Key(Space) ], "advance": [ Key(Space) ] },
    axes:    { "move_x": ( neg: [ Key(A), Key(Left) ], pos: [ Key(D), Key(Right) ] ) },
)
```

A system reads `action("jump")`, never a key code.

### Game-provided systems

Systems come from the game's `.hom` module and are registered against the
component types they read. The engine ships only the systems its own core
components require.

| Engine system | Reads |
|---|---|
| transform propagation | `Parent`, `Transform` |
| message delivery | `Port` |

Every other system belongs to a game.

A system is a Homun lambda. Its parameter list is its query: `x::T` binds a
component mutably, `x: T` immutably, and `dt: float` is the tick delta.

```
player_system := (t::Transform, p::PlayerControlled, dt: float) -> _ { .. }
```

### Script-facing API

The surface the engine exposes to `.hom`. Everything outside it is engine
internals.

| Module | Provides |
|---|---|
| `engine` | `Transform`, `Vec2`, `Vec3`, `Collider`, `CellPos` |
| `engine::math` | `cos`, `sin`, `sqrt`, `floor` — `use std` carries only `abs`, `min`, `max`, `clamp` |
| `engine::input` | `action(name) -> bool` held, `action_pressed(name) -> bool` one-shot |
| `engine::world` | `overlapping(a, b)` collider pairs by component name, `restart()`, `spawn`, `despawn` |

`engine::input` never exposes a key code. `engine::world::overlapping` is the
only pair query; single-entity systems cannot see other entities.

## Pipelines

| | **DataPipeline** | **View** |
|---|---|---|
| Is | `hecs::World` + systems | a component query → a frame |
| Owns | object state | nothing; read-only |
| Runs on | CPU, parallelisable | GPU (3D/2D), CPU (Unicode) |
| Input | scene file, input events, dt | World query, camera |
| Output | mutated World | a frame |

State lives only in the DataPipeline. Views never write to a World.

## Runtime model

`hecs::World` is the runtime model. `Vec<Node>` in RON is serialization only;
the boundary is crossed at load and at save.

- `Transform` is local. `GlobalTransform` is a separate component, derived by a
  propagation system from `Transform` + `Parent`, read-only, not serialized.
- Sibling order is explicit: `SiblingIndex(u32)`.
- `Parent(Entity)` carries hierarchy. RON `Node.children` maps to `Parent` on
  load; `Parent` + `SiblingIndex` reconstruct the tree on save.

## Components

| Component | Type | Read by |
|---|---|---|
| `Name` | `String`, unique per World | `ViewRef` resolution |
| `Transform` | translation, rotation, scale; local | propagation |
| `GlobalTransform` | `Mat4`; derived, not serialized | all Views |
| `Parent` | `Entity`; absent = root | propagation |
| `SiblingIndex` | `u32` | save |
| `MeshRef` | path | `View3D` |
| `Sprite` | sheet path or `ViewRef`, grid, cell, size | `View2D` |
| `Tilemap` | tileset, tile_size, cells | `View2D` |
| `ColorRect` | size, color | `View2D` |
| `CellPos` | `(i32, i32)` | `ViewText` |
| `CharArt` | art or file, fg, bg, anchor | `ViewText` |
| `Layer` | `i32`; draw order within a View | all Views |
| `Camera` | projection or `CellViewport` | the View on the same entity |
| `View` | world, camera, kind, stages, out, size, update | render scheduler |
| `Collider` | half extents, shape | a game's collision system |
| `Port` | `MessagePort` | message delivery |

These are the engine's core components. Game components are registered by the
game's `.hom` module and stored under their type name.

## View

```rust
pub struct View {
    pub world:  WorldId,      // the World to render; may differ from the host World
    pub camera: CameraSource, // SelfEntity | Entity(EntityId) | Inline(Camera)
    pub kind:   ViewKind,     // View3D | View2D | ViewText
    pub stages: Vec<Stage>,
    pub out:    OutKind,      // Pixels | Cells
    pub size:   Extent,       // Pixels(w, h) | Cells(cols, rows)
    pub update: Update,       // Always | Once | Never
}
```

| View kind | Queries | Natural output | Samples textures as |
|---|---|---|---|
| `View3D` | `(GlobalTransform, MeshRef)` | `Pixels<Native>` | `Pixels` |
| `View2D` | `(GlobalTransform, Sprite \| Tilemap \| ColorRect)` | `Pixels<Native>` | `Pixels` |
| `ViewText` | `(CellPos \| GlobalTransform, CharArt)` | `Cells<Native>` | `Cells` |

- `out` and `size` state how drawing happens. `stages` bridge the natural output
  to `out`.
- A View retains its last frame. `update` gates re-rendering, not the World's
  tick.
- The camera is always expressed in the target World's coordinate space:
  - `SelfEntity` — `Camera` + `GlobalTransform` on the View's own entity. Valid
    only when `world` is the View's host World; a follow-cam is this plus
    `Parent`.
  - `Entity(id)` — a camera entity in the target World.
  - `Inline(Camera)` — an explicit camera in the target World's space.

  `SelfEntity` with a foreign `world` is a load error.
- A View's kind is declared. It is never inferred.
- An entity not matching a View's query is not drawn by it. Warning at load,
  never an error.
- An entity may carry components for more than one View kind. Changing a View's
  kind changes the query; the World is untouched.
- `ViewText` reads `CellPos` when present, otherwise quantises
  `GlobalTransform` to cells.
- Many Views may render one World: split-screen, rear-view mirror, security
  camera. They share the World; no state is copied.
- One World may host Views that render other Worlds: an arcade cabinet, an
  embedded mini-game, a UI sandbox.
- `size` is in the View's own `out` unit. Upstream resolution is derived: a
  `Cells(40, 20)` output through `ToPixels { cell: (8, 16) }` needs a 320×320
  pixel render; a `Pixels(w, h)` output through `ToCells` needs ⌈w/2⌉×⌈h/4⌉ cells.

## Frames and stages

Two frame types. Provenance is part of the type.

```rust
pub struct Pixels<Origin>(/* .. */, PhantomData<Origin>);
pub struct Cells <Origin>(/* .. */, PhantomData<Origin>);

pub struct Native;     // produced directly by a View of that frame type
pub struct Converted;  // produced by a stage from the other frame type
```

Two stages, forming a bijection between the frame types.

```rust
ToCells <V: ViewOut<Out = Pixels<Native>>> : Out = Cells <Converted>
ToPixels<V: ViewOut<Out = Cells <Native>>> : Out = Pixels<Converted>
```

`ToCells` takes a `GlyphSet`: `Mixed` | `Quadrant` | `Braille`.
`ToPixels` takes a font atlas resource and a cell pixel size.

Each stage accepts `Native` input only, so a stage pair cannot be chained into a
lossy round-trip. `ToPixels::forced` accepts any `Cells<_>` for the deliberate
case.

Origin is erased across a View: `Pixels<Converted>` consumed as a material
yields `Pixels<Native>`.

### Composition closure

| producer output \ consumer samples | `Pixels` | `Cells` |
|---|---|---|
| `Pixels` | direct | `ToCells` |
| `Cells` | `ToPixels` | direct |

A mismatch with no stage is a load error naming both types.

## Cross-World

Three mechanisms, and only these:

- a **View's `world`** — a View hosted in one World renders another;
- a **`ViewRef`** — an entity samples a View's texture regardless of which World
  either lives in;
- a **`MessagePort`** — typed messages, bounded queue, no shared references.

There is no shared entity access between Worlds. A View reads its target World;
it never writes to it.

## Execution

The render graph is a DAG: a View depends on the tick of the World it renders,
and on every View it samples through a `ViewRef`.

1. Tick each World whose `update` allows it. Disjoint Worlds may tick in
   parallel. A skipped World receives no dt.
2. Render Views in topological order. Views over one World are read-only and may
   render in parallel.
3. Apply each View's stages to its output.

A cycle in the graph resolves to the previous frame. Depth is capped;
scene-file reference cycles are a load error.

Cost:

- Readbacks equal the number of `ToCells` stages.
- A `Pixels`-only subgraph is one command encoder and one submit: producer passes
  recorded before consumer passes, produced textures sampled directly.
- A `Cells`-only subgraph performs no GPU work.
- All `Pixels` Views share one wgpu device and queue.

## Root

The surface requires a frame type; the root View must produce it.

```rust
shinra::tui(ToCells::mixed(View2D::new(path)))            // 2D      -> Cells
shinra::tui(ToCells::braille(View3D::new(path)))          // 3D      -> Cells
shinra::tui(ViewText::new(path))                       // Unicode -> Cells
shinra::gui(ToPixels::default(ViewText::new(path)))  // Unicode -> Pixels
```

`shinra::tui` requires `Out = Cells<_>` and sizes the root View to the terminal.
`shinra::gui` requires `Out = Pixels<_>`.

Stages are always written explicitly; they carry parameters that are not
inferable. There is no game-level presentation config. The `ToCells` `GlyphSet`
is a viewer preference and lives in `ide.ron`.

### IDE viewport

The Viewport panel requires a frame type, following the panel's mode.

| Game | Panel requires `Cells` | Panel requires `Pixels` |
|---|---|---|
| `View3D` / `View2D` | `ToCells` | direct |
| `ViewText` | direct | `ToPixels` |

For a `ViewText` game in a terminal there is no stage and no `GlyphSet` to
cycle; panel size is the cell grid directly. For a `Pixels`-producing View the
panel drives a 2×4-subpixel render target.

The editor camera is a `Camera` the IDE owns, not the scene's `game_camera`: a
projection for `View3D` / `View2D`, a `CellViewport` with integer scrolling and
no damping for `ViewText`.

## Systems

Systems are component-driven: the presence of a component registers its system.
Registration is uniform for engine and game systems; only the source differs.

The engine's own systems are transform propagation (`Parent`, `Transform`) and
message delivery (`Port`). Every other system is a game's, declared in `.hom`.

No system reads a rendering component. Collision reads `Collider`.

## File conventions

| | Extension |
|---|---|
| Scene | `*.tscn.ron` |
| Resource | `*.tres.ron` |

Resources: tileset, char-art sheet, font atlas, visual set.

```ron
// assets/fonts/cp437.tres.ron
( name: "cp437", atlas: "assets/fonts/cp437.png", cell: (8, 16), charset: Cp437 )
```

The font atlas is a baked PNG over the closed character set.

### Text art

| | PNG sprite | Text art |
|---|---|---|
| Shape | alpha | character; space is transparent |
| Colour | RGB per pixel | palette index per cell |
| Scaling | any | none; cells are indivisible |
| Animation | adjacent sheet cells | frame list |

v1: one character plane, per-block `fg` / `bg`.
v2: optional same-size colour plane plus palette.

Character art may be inline (`art: ["...", "..."]`) or an external `.txt` file.
`transparent` defaults to space.

## Error surfaces

| Class | Caught at |
|---|---|
| Illegal stage composition | compile |
| Adjacent stage round-trip | compile |
| Root frame type mismatch | compile |
| Missing stage between producer and consumer | load |
| Scene-file reference cycle | load |
| Depth cap exceeded | load |
| Declared View kind unsupported by scene | load |
| Entity matching no View query | warning |
| Cross-View round-trip | warning |

## Boundaries

- `ratatui` owns IDE chrome: borders, lists, scroll state, text wrapping,
  `unicode-width`, PTY vt100. Only the Viewport panel hosts a View.
- `core/overlay.rs` is a `ViewText` output composited over the root.
- `hecs` is a DataPipeline concern. It does not appear in the `View` trait.
- Scene trees are RON. View composition, component types and systems are `hom`.
- The engine crate holds no game type, no game rule and no tuning constant.

## Status

| | State |
|---|---|
| `View3D` | exists |
| `ToCells` (`textart.rs`) | exists |
| `Presenter` / `FrameCtx` | exists |
| `hecs` World | exists; rebuilt per frame, flattened |
| `View2D` | to build |
| `ViewText`, `CellGrid` | to build |
| `ToPixels`, font atlas resource | to build |
| `View` component, `ViewRef`, render graph | to build |
| Provenance types | to build |
| `Parent`, `GlobalTransform`, `SiblingIndex` | to build |
| World → RON serialization | to build |
| `MessagePort` | to build |
| Component registry | to build |
| Script-facing API | to build |
| `.hom` game logic | written, not compiling |
| Action maps | to build |
| `homunc` in-repo | not started |

### Examples

| Game | View | Needs | `.hom` |
|---|---|---|---|
| game1 `bunny` | `View3D` | — | `orbit_camera.hom` |
| game2 `teapot` | `View3D` | — | `orbit_camera.hom` |
| game3 `dino-run` | `View2D` | `ColorRect`, `Layer` | `player.hom`, `scroller.hom`, `obstacle.hom` |
| game4 `terminal-hearts` | `View2D` | `ColorRect`, `Layer` | `dialogue.hom` |

Each game also carries an `input.tres.ron` action map. These files exist and
define the target behaviour; they do not compile until `homunc` is in-repo and
the script-facing API is implemented.

What each replaces:

| `.hom` | Replaces |
|---|---|
| `orbit_camera.hom` | `engine/src/scene.rs:89` `orbit_eye`, and `BunnyTag` / `TeapotTag` at `:61-62` |
| `player.hom` | `GRAVITY`, `JUMP_VELOCITY` and the jump branch in `frontend/tui/src/core/run.rs` |
| `scroller.hom` | the `ScrollX` arm of `run.rs` |
| `obstacle.hom` | `HITBOX_HALF` and the collision branch of `run.rs` |
| `dialogue.hom` | the dialogue index handling in `run.rs` |
| `input.tres.ron` | the fixed five fields of `abi/src/lib.rs` `InputFrame` |

After the move, `engine` and `frontend/tui` contain no game type, rule or
constant, and `scene/src/lib.rs` `ComponentValue` is gone.

### Order of work

0. Component registry; named input actions; move every game rule out of
   `engine` and `frontend/tui` into `shinra-examples/**/*.hom`. Requires
   `homunc` in-repo.
1. `Parent`, `GlobalTransform`, `SiblingIndex`; load once per scene; World → RON save.
2. `View` component, `ViewRef`, provenance types, render graph; existing pass
   becomes `View3D`.
3. `ViewText`, `CellGrid`, character-art authoring, font atlas, `ToPixels`;
   `overlay.rs` moves onto it.
4. `View2D` with pixel-snapped orthographic camera, `ColorRect`, tile atlas.
5. `MessagePort`; interactive Views.

Steps 1 and 2 leave every snapshot baseline unchanged:
`frontend/tests/snapshots/`, `engine/tests/snapshot_test.rs`,
`engine/tests/frontend_parity_test.rs`.

## Open questions

- **Camera attachment.** `attach { node, offset, look_at, damping }` for
  follow-cams. Damping is `alpha = 1 - exp(-dt / tau)`. Disabled for
  `ViewText`.
- **Deferred composite.** `composite: Immediate | Deferred` to composite an
  authored char-art output in cell space after a root `ToCells`. Requires the host
  surface to face the camera within a tolerance.
- **Interactive Views.** Hierarchical input focus for playable Views.
- **`MessagePort` shape.** Message types, queue bound, delivery ordering,
  behaviour on a full queue.
- **Split-screen root.** Whether the surface takes several root Views with a
  layout, or one View whose sub-rects are written by several cameras.
- **Binding resolution order.** Panel focus before global bindings.
- **`hom` boundary.** Confirm the RON / `hom` split once `homunc` is in-repo.

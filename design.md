# Shinra Engine — Design

## Concepts

| | Is |
|---|---|
| **Render unit** | a `world` or a `canvas`: the thing a View renders |
| **World** | entities and components in space. No camera |
| **Canvas** | a 2D tree, in surface units. UI lives here. No camera |
| **View** | points at a render unit, holds a camera, produces a texture |
| **Stage** | a conversion between the two texture types |

One pointing relation, one level each:

```
game.ron ──> views ──> render units   (world | canvas)
```

`game.ron` names views and nothing else. A view names one render unit. A render
unit's objects name views as textures. The view named `main` is the root.

A View is independent: it belongs to neither the render unit it targets nor the
one that samples it. Objects reach a View's output the same way they reach an
image — as a texture.

Usually the outermost render unit is a canvas, with a node whose texture is a
View of a world. That is what makes UI ordinary: the HUD is a sibling of the
game picture, not something layered onto it by the host.

## Files

A subtree is written inline or replaced by a reference:

```
Node        = inline | Ref("path.ron")
RenderUnit  = Ref("world.ron") | Ref("canvas.ron")
```

Convention: one render unit per file, because a render unit is a View's unit of
work. Splitting further is the author's choice, not the format's requirement.

```
assets/games/game3/
  game.ron          views, root render unit, action map
  canvas.ron        the screen: the game picture plus its HUD
  world.ron         entities and components
  input.tres.ron    action map
  player.hom  scroller.hom  obstacle.hom
```

## `game.ron`

Every View the game needs, in one place, so its rendering cost is legible at a
glance. `main` is the root.

```ron
(
    name:  "dino-run",
    input: "input.tres.ron",

    views: {
        "main": (
            unit:     Ref("canvas.ron"),
            graphics: View2D,
            camera:   ( projection: Screen ),
            size:     Fill,
        ),
        "game": (
            unit:     Ref("world.ron"),
            graphics: View2D,
            camera:   ( projection: Orthographic( half_height: 2.6 ) ),
            size:     Fill,
        ),
        "mirror": (
            unit:     Ref("world.ron"),
            graphics: View3D,
            camera:   (
                projection: Perspective( fov_y_degrees: 60.0, znear: 0.1, zfar: 100.0 ),
                anchor: Some(( node: "rear_mirror", offset: (0.0, 0.0, 0.0), facing: Backward )),
            ),
            size:     Pixels(256, 128),
        ),
    },
)
```

## World

Entities with components. No camera: where a world is seen from is a View's
business, and the player's.

```ron
World((
    name: "dino-run",
    nodes: [
        (
            name: "dino",
            transform: ( translation: (-3.0, 0.0, 0.2), rotation: (0.0, 0.0, 0.0, 1.0), scale: (1.0, 1.0, 1.0) ),
            sprite:   Some(( source: Png("assets/images/2x2_grid.png"), grid: (2, 2), cell: (0, 0), size: (1.2, 1.2) )),
            collider: Some(( shape: Box, size: (0.84, 0.84) )),
            components: {
                "PlayerControlled": ( gravity: -32.0, jump_velocity: 10.5, vy: 0.0 ),
            },
        ),
        (
            name: "rear_mirror",
            transform: ( … ),
            sprite: Some(( source: View("mirror"), size: (0.6, 0.2) )),
        ),
    ],
))
```

Node fields: `transform`, `sprite`, `mesh`, `tilemap`, `collider`, `components`,
`children`. A node with none of the visual fields draws nothing and is exempt
from every visual check.

## Canvas

A 2D tree in surface units. Layout replaces a transform: a canvas node is placed
by rule, not by a matrix.

```ron
Canvas((
    name: "dino-run",
    nodes: [
        ( name: "viewport", rect: ( fill: true ),
          sprite: Some(( source: View("game") )) ),

        ( name: "score_bg", rect: ( anchor: TopRight, size: (16, 1), offset: (-1, 1) ),
          color_rect: Some(( color: (0.0, 0.0, 0.0, 0.5) )) ),

        ( name: "score", rect: ( anchor: TopRight, offset: (-2, 1) ),
          text: Some(( from: "Run.crashes", format: "crashes: {}" )) ),
    ],
))
```

Node fields: `rect`, `sprite`, `text`, `color_rect`, `children`.

`rect`: `anchor` (`TopLeft` … `BottomRight`, `Center`), `offset`, `size`, `fill`.

A canvas is rendered by a view whose camera projection is `Screen`: one canvas
unit to one surface unit, no transform.

`text.from` names a component field in a world, so a HUD reads live state
without a system pushing it.

## View

```rust
pub struct View {
    pub unit:     RenderUnit,   // Ref("world.ron") | Ref("canvas.ron")
    pub graphics: GraphicsKind, // View3D | View2D | ViewText
    pub camera:   Camera,
    pub stages:   Vec<Stage>,
    pub size:     Extent,       // Fill | Pixels(w, h) | Cells(cols, rows)
    pub update:   Update,       // Always | Once | Never
}

pub struct Camera {
    pub projection: Projection,        // Perspective | Orthographic | CellViewport | Screen
    pub anchor: Option<Anchor>,        // absolute when None
}

pub struct Anchor {
    pub node:   String,   // a node in the target unit; the View reads it, it is not one
    pub offset: [f32; 3],
    pub facing: Facing,   // Forward | Backward | Target([f32; 3])
}
```

- A View retains its last texture. `update` gates re-rendering, not the target's
  tick.
- `anchor` lets a camera follow a node — a mirror on a car, a camera behind a
  player — by reference, without the View being an entity.
- Many Views may target one render unit: split-screen, a mirror, a security
  camera. Nothing is copied.
- `size: Fill` takes the size of whatever samples it; the root View fills the
  surface.

| Graphics kind | Query | Natural texture |
|---|---|---|
| `View3D` | `(GlobalTransform, MeshRef)` | `Pixels<Native>` |
| `View2D` | `(GlobalTransform, Sprite \| Tilemap \| ColorRect)` | `Pixels<Native>` |
| `ViewText` | `(CellPos \| GlobalTransform, CharArt)` | `Cells<Native>` |

An entity matching no query is not drawn by that View. A warning at load, never
an error. `ViewText` reads `CellPos` when present, otherwise quantises
`GlobalTransform`.

## Textures

```
source = Png(path) | View(name)
```

A View named in `game.ron` is referenced by name, so one View serves any number
of objects.

**Self-reference resolves to the previous frame.** A mirror is a node in the
world its own View renders, so that View sees the mirror; the mirror samples last
frame's texture. This is the only cycle the format permits, and it is bounded.

## Ownership

The engine provides capability. A game provides its rules, in `.hom`, in its own
project. The engine holds no game rule and no tuning constant.

| Engine | Game |
|---|---|
| render units, views, stages, render graph | its own component types |
| `Transform`, `Collider`, transform propagation | gravity, jumping, movement, scrolling |
| collision **detection** | collision **response** |
| raw keys → named actions and axes | which key, how much, how fast |
| asset loading, scene serialization, scheduler | dialogue flow, win and lose rules |

Motion is entirely the game's: the engine integrates nothing, so it never
inserts a step into the middle of a frame.

## DataPipeline

`hecs::World` is the runtime model. RON is serialization only, crossed at load
and at save.

- `Transform` is local. `GlobalTransform` is derived by a propagation system from
  `Transform` + `Parent`; read-only, not serialized.
- `Parent(Entity)` carries hierarchy; `SiblingIndex(u32)` makes save order
  stable.
- Game components are stored opaquely by type name and resolved by the game's
  compiled module. An unregistered name is a load error.
- Mutated components are written back to their node, so a HUD and Ctrl+S see
  live state.

Systems are component-driven: a component's presence registers its system. A
system is a Homun lambda whose parameter list is its query — `x::T` binds
mutably, `x: T` immutably, `dt: float` is the tick delta. The engine's only
systems are transform propagation and message delivery.

## Collision

Detection is the engine's; response is the game's.

```rust
pub enum Shape { Box, Ellipse }   // both axis-aligned
pub struct Collider { shape, size, offset }   // size is the bounding box, either shape
pub struct Hit { normal: [f32; 2], depth: f32 }   // how to move `a` off `b`
```

| Pair | Method | Depth |
|---|---|---|
| Box ↔ Box | least-penetration axis | exact |
| Box ↔ Ellipse | divide by the radii: a unit circle against an axis-aligned box | estimate |
| Ellipse ↔ Ellipse | divide by the first's radii, then Newton for the closest point | estimate |

Bounding boxes are the broadphase, so the cheap pre-test does not depend on
shape. Depth is exact only for box↔box: an anisotropic scale does not preserve
distance, so ellipse depths push out of a penetration but do not settle a
resting contact.

`overlapping(a, b)` is a live query against current positions, not a snapshot
taken before the game moved anything. It returns hits naming the other node, so
a game can tell one obstacle from another.

## Input

The engine emits raw keys and resolves them against the game's map. It binds no
key to any meaning.

```ron
(
    actions: { "jump": ["Space"] },
    axes:    { "move_x": ( neg: ["A", "Left"], pos: ["D", "Right"] ) },
)
```

`action(name)` held, `action_pressed(name)` one-shot, `axis(name)` in −1..=1 with
opposing keys cancelling.

## Frames and stages

Two texture types. Origin is part of the type.

```rust
pub struct Pixels<Origin>;
pub struct Cells<Origin>;
pub struct Native;      // produced directly by a View of that type
pub struct Converted;   // produced by a stage from the other type

ToCells <V: ViewOut<Out = Pixels<Native>>> : Out = Cells <Converted>
ToPixels<V: ViewOut<Out = Cells <Native>>> : Out = Pixels<Converted>
```

`ToCells` takes a `GlyphSet` (`Mixed` | `Quadrant` | `Braille`); `ToPixels` takes
a font atlas and a cell pixel size. Each accepts `Native` only, so a stage pair
cannot be chained into a lossy round-trip; `ToPixels::forced` exists for the
deliberate case. Origin is erased across a View.

| producer \ consumer samples | `Pixels` | `Cells` |
|---|---|---|
| `Pixels` | direct | `ToCells` |
| `Cells` | `ToPixels` | direct |

A mismatch with no stage is a load error naming both types.

## Execution

The render graph is a DAG: a View depends on the tick of the unit it renders and
on every View it samples.

1. Tick each world whose `update` allows it. Disjoint worlds may tick in
   parallel; a skipped world receives no dt.
2. Render Views in topological order. Views over one unit are read-only and may
   render in parallel.
3. Apply each View's stages to its texture.

A cycle resolves to the previous frame. Depth is capped; a file-reference cycle
is a load error.

Cost: readbacks equal the number of `ToCells` stages. A `Pixels`-only subgraph is
one command encoder and one submit. A `Cells`-only subgraph performs no GPU work,
so a text-only game needs no GPU driver. All `Pixels` Views share one wgpu device.

## Errors

| Class | Caught at |
|---|---|
| Illegal stage composition | compile |
| Adjacent stage round-trip | compile |
| Surface texture type mismatch | compile |
| Missing stage between producer and consumer | load |
| File-reference cycle, depth cap | load |
| Unregistered component | load |
| `View(name)` naming no view in `game.ron` | load |
| No view named `main` | load |
| `anchor.node` naming no node, or ambiguous | load |
| Entity matching no View query | warning |

## Boundaries

- `ratatui` owns IDE chrome: borders, lists, scroll state, text wrapping, PTY
  vt100. The Viewport panel hosts a View; the other panels do not.
- `hecs` is a DataPipeline concern and does not appear in the `View` trait.
- Render units are RON. View composition, component types and systems are `hom`.
- The engine crate holds no game type, rule or tuning constant.

## Status

| | State |
|---|---|
| `View3D` (fused mesh + sprite pass) | exists as `Engine::render` |
| `ToCells` | exists as `TextArtGpu` |
| Collision detection, Box + Ellipse | exists as `engine::collide` |
| Script API, module loader, v2 ABI | exists |
| `shinra build`: `.hom` → cdylib, glue generated | exists |
| `hecs` World | exists; rebuilt per frame, flattened |
| Canvas, `rect`, `text` | to build |
| `game.ron`, named views, `Ref` | to build |
| `View2D`, `ViewText`, `CellGrid` | to build |
| `ToPixels`, font atlas | to build |
| Live `overlapping`, `Hit` with identity | to build |
| `axis()` | to build |
| `Parent`, `GlobalTransform`, `SiblingIndex` | to build |
| World → RON serialization | to build |

Legacy to remove: `runner/`, `engine::input::Keymap` and the v1 `InputFrame` bake
a specific control scheme into the engine, and nothing produces v1 modules any
more.

### Order of work

1. Remove the legacy input path and `runner`.
2. `axis()`; live `overlapping` returning `Hit` with the other node's name.
3. `game.ron` with named views and `Ref`; split each game's scene into a world
   and a canvas.
4. Canvas primitives: `rect`, `text`, `color_rect`. `overlay.rs` becomes canvas
   nodes.
5. `Parent`, `GlobalTransform`, `SiblingIndex`; load once per scene; save.
6. `ViewText`, `CellGrid`, char-art authoring, font atlas, `ToPixels`.
7. `View2D` with a pixel-snapped orthographic camera; tile atlas.

## Open questions

- **Canvas primitives beyond these four** — nine-slice, scrolling containers,
  and whether `text` needs wrapping and alignment.
- **`text.from`** — how a canvas addresses a component field in a world it does
  not contain.
- **System order within a game.** Component-driven registration says nothing
  about order, and the generated order is currently alphabetical by file. An
  `@stage(...)` attribute is the likeliest answer.
- **Per-object one-off behaviour.** A `Script` component naming a `.hom` with an
  `on_tick(node, dt)` convention, versus requiring a component per behaviour.
- **`MessagePort`** between worlds: message types, queue bound, ordering.
- **`.to_string()` on string literals** in Homun argument position keeps the
  engine's script API allocating; see `Homun-Lang/report.md`.

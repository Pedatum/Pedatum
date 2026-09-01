# Shinra Engine

## Design

```
             bundle.so
         data.rs + buffer.rs
        ↙        ↓       ↘
   data ──►  game.so ──┬──►  process.so ────►  data
  status     control   │         ecs          update
                ↓      │
            asset.so ──┴──►  render.so ───►  buffer ──►  show
            appearance         graphs        targets

  └──── imperative ────┘└───── pure ───────┘
```


ecs只是其中一種實現 我們也可以使用不同方式實踐data update

這個設計主要是為了hot module replace 任何組件

比如改變render.so就能改變遊戲畫面邏輯(ex: 畫面風格)
替換asset.so就能改變遊戲的主題

game.so 可以從data status s/l 去載入存檔遊戲進度

game.so本身也可以hot module替換 改變遊戲操作邏輯 用libloading deload previous so

但是所有一切 都必須遵守bundle.so contract

改變bundle.so contract 等於替換game sets 需要全部重load 

這是為了實現gametok gametok 是 tiktok概念的minigame平台 右滑會換遊戲



## Data

```rust
#[repr(C)]
#[derive(se::Schema)]
pub struct Transform { pub pos: [f32; 3], pub rot: [f32; 4], pub scale: [f32; 3] }
```



## Process

A stage is a function whose parameter list *is* its query. Reaching outside the
signature is not expressible.

```rust
fn motion(t: &mut Transform, b: &mut Body, i: &Input, dt: f32) { … }
```


## Render

DAG架構 wgsl pipeline 用以render such multi view, mirror,......

pure render from data + asset


## View buffer as texture

To prevent complex render graph flow

texture can choose a view buffer defined at buffer.rs

But it always fetch from previous frame (deep mirror will have wave move as slow light)


## Control

`game.so` is the only station that knows the time. It owns **time, input, and
which module fills which slot** — and nothing else may touch those.

Its write surface is narrow, or single-writer reasoning dies:

| | |
|---|---|
| the input components | a stage only ever takes `&Input` |
| spawn / despawn | reconciling what should exist against what does |

Content never writes data. An `asset.so` carrying a fifth character adds a
*definition*; a reconcile stage spawns. A four-character save against five
definitions gains the fifth with default state — status is what happened, the
roster is what exists.

## Layout

**A `.rs` at depth 1 is a module. Anything deeper is a source file inside one.**

```
game1/
├── bundle/  data.rs  buffer.rs          both required
├── process/ motion.rs  collide.rs
├── render/  graph1.rs  graph2.rs  graph1/common.rs
├── asset/   bunny.rs  teapot.rs  bunny/model.obj
└── game/    game1.rs  game2.rs
```

A folder named after a sibling `.rs` is private to it; one without is shared
across the category.

One entry point each:

```
bundle.so     se_register_layouts()    name → layout       (data.rs)
              se_register_buffers()    name → shape, count, sampled  (buffer.rs)
asset/*.so    se_register_assets()     name → bytes
process/*.so  se_register_stages()     spec + function
render/*.so   se_register_graph()      nodes + edges
game/*.so     se_register_control()    + tick
```

`bundle/` is the one folder that is not one module per `.rs`: `data.rs` and
`buffer.rs` compile together into a single `bundle.so`. There is no `data.so` —
layouts and buffers are the same contract, so they version and reload as one.


## IDE

that all render pipelie will have render as tui
// Homun shim. `homunc --extern engine` makes each game module reference this
// one, so the whole crate shares these types.
//
// Nothing is redefined here. `Transform.translation` is a `[f32; 3]` and a
// script indexes it directly — `t.translation[1] := ...` — so there is no
// script-side copy of the type and nothing to convert per entity per frame.

pub use scene::Collider;
pub use scene::Transform;
pub use serde::Deserialize;
pub use serde::Serialize;

// Homun's codegen appends `.to_string()` to string literals passed as
// arguments, so the script-facing signatures take String.
pub fn action(name: String) -> bool {
    shinra_engine::script::action(&name)
}
pub fn action_pressed(name: String) -> bool {
    shinra_engine::script::action_pressed(&name)
}
pub fn axis(name: String) -> f32 {
    shinra_engine::script::axis(&name)
}
/// One contact, in the shape a script wants: the other object's name and how to
/// push out of it. Flat floats because a script reads `h.normal_x`, not an array.
#[derive(Clone, Debug, Default)]
pub struct Hit {
    pub other: String,
    pub normal_x: f32,
    pub normal_y: f32,
    pub depth: f32,
}

pub fn overlapping(a: String, b: String) -> Vec<Hit> {
    shinra_engine::script::overlapping(&a, &b)
        .into_iter()
        .map(|h| Hit {
            other: h.b_node,
            normal_x: h.normal[0],
            normal_y: h.normal[1],
            depth: h.depth,
        })
        .collect()
}
pub fn restart() {
    shinra_engine::script::restart()
}

pub use shinra_engine::script::math::cos;
pub use shinra_engine::script::math::floor;
pub use shinra_engine::script::math::sin;
pub use shinra_engine::script::math::sqrt;

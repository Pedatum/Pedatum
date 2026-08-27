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
pub fn overlapping(a: String, b: String) -> Vec<(String, String)> {
    shinra_engine::script::overlapping(&a, &b)
}
pub fn restart() {
    shinra_engine::script::restart()
}

pub use shinra_engine::script::math::cos;
pub use shinra_engine::script::math::floor;
pub use shinra_engine::script::math::sin;
pub use shinra_engine::script::math::sqrt;

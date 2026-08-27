//! The Shinra scene format.
//!
//! Three kinds of file, and one pointing relation between them:
//!
//! ```text
//! game.ron ──> views ──> render units (world.ron | canvas.ron)
//! ```
//!
//! A render unit holds no camera: where it is seen from belongs to the view
//! that draws it.

pub mod camera;
pub mod flat;
pub mod canvas;
pub mod game;
pub mod tileset;
pub mod world;

pub use camera::{Anchor, Camera, Facing, Projection};
pub use flat::{flatten, global_transform, unflatten, FlatNode};
pub use canvas::{Anchor as RectAnchor, Canvas, CanvasNode, CanvasSprite, ColorRect, Rect, Text};
pub use game::{Extent, Game, GlyphSet, GraphicsKind, OutputKind, Ref, Stage, Update, ROOT_VIEW};
pub use tileset::{Tile, Tileset};
pub use world::{Cell, Collider, MeshRef, Node, Scene, Shape, Sprite, Tilemap, Transform};

use serde::{Deserialize, Serialize};

/// Where an object's texture comes from. A view's output is reached the same
/// way an image is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TexSource {
    Png(String),
    /// Names a view declared in `game.ron`.
    View(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texture_source_roundtrips() {
        for src in [
            TexSource::Png("assets/images/sheet.png".into()),
            TexSource::View("mirror".into()),
        ] {
            let text = ron::to_string(&src).unwrap();
            assert_eq!(ron::from_str::<TexSource>(&text).unwrap(), src);
        }
    }
}

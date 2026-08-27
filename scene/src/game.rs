//! `game.ron`: the views a game needs, and nothing else.
//!
//! One pointing relation, one level each:
//!
//! ```text
//! game.ron ──> views ──> render units (world | canvas)
//! ```
//!
//! A view is independent: it belongs to neither the render unit it targets nor
//! the one that samples it. The view named `main` is the root.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Camera;

/// A path to another RON file. Anywhere a subtree may appear, it can be
/// replaced by one of these.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ref(pub String);

/// The name every game's root view must use.
pub const ROOT_VIEW: &str = "main";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Game {
    pub name: String,
    /// Path to the action map.
    pub input: String,
    pub views: BTreeMap<String, ViewDef>,
}

impl Game {
    /// The root view, or `None` when the game declares no `main`.
    pub fn root(&self) -> Option<&ViewDef> {
        self.views.get(ROOT_VIEW)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewDef {
    /// The render unit this view draws.
    pub unit: Ref,
    pub graphics: GraphicsKind,
    pub camera: Camera,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub size: Extent,
    #[serde(default)]
    pub update: Update,
}

/// Which component query a view runs, and therefore what it can draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsKind {
    View3D,
    View2D,
    ViewText,
}

impl GraphicsKind {
    /// The texture type this kind produces before any stage.
    pub fn natural_output(self) -> OutputKind {
        match self {
            Self::View3D | Self::View2D => OutputKind::Pixels,
            Self::ViewText => OutputKind::Cells,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKind {
    Pixels,
    Cells,
}

/// A conversion between the two texture types.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Stage {
    /// Pixels to cells, by thresholding.
    ToCells(GlyphSet),
    /// Cells to pixels, by drawing each character from a font atlas.
    ToPixels { font: String, cell: [u32; 2] },
}

impl Stage {
    pub fn input(&self) -> OutputKind {
        match self {
            Self::ToCells(_) => OutputKind::Pixels,
            Self::ToPixels { .. } => OutputKind::Cells,
        }
    }

    pub fn output(&self) -> OutputKind {
        match self {
            Self::ToCells(_) => OutputKind::Cells,
            Self::ToPixels { .. } => OutputKind::Pixels,
        }
    }
}

/// Which characters `ToCells` may emit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlyphSet {
    #[default]
    Mixed,
    Quadrant,
    Braille,
}

/// A view's texture size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Extent {
    /// Take the size of whatever samples this view; the root fills the surface.
    #[default]
    Fill,
    Pixels(u32, u32),
    Cells(u32, u32),
}

/// When a view re-renders. Gates the view, never the render unit's tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    #[default]
    Always,
    Once,
    Never,
}

impl ViewDef {
    /// What this view hands to whatever samples it, after its stages.
    pub fn output(&self) -> OutputKind {
        self.stages
            .last()
            .map(|s| s.output())
            .unwrap_or_else(|| self.graphics.natural_output())
    }

    /// Is the stage chain well formed for this view's graphics kind?
    pub fn stages_fit(&self) -> Result<(), String> {
        let mut have = self.graphics.natural_output();
        for (i, stage) in self.stages.iter().enumerate() {
            if stage.input() != have {
                return Err(format!(
                    "stage {i} takes {:?} but the chain has {:?}",
                    stage.input(),
                    have
                ));
            }
            have = stage.output();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Camera, Projection};

    fn view(graphics: GraphicsKind, stages: Vec<Stage>) -> ViewDef {
        ViewDef {
            unit: Ref("world.ron".into()),
            graphics,
            camera: Camera {
                projection: Projection::Screen,
                anchor: None,
            },
            stages,
            size: Extent::Fill,
            update: Update::Always,
        }
    }

    #[test]
    fn a_view_without_stages_hands_over_its_natural_output() {
        assert_eq!(
            view(GraphicsKind::View3D, vec![]).output(),
            OutputKind::Pixels
        );
        assert_eq!(
            view(GraphicsKind::ViewText, vec![]).output(),
            OutputKind::Cells
        );
    }

    #[test]
    fn a_stage_changes_what_a_view_hands_over() {
        let v = view(
            GraphicsKind::View3D,
            vec![Stage::ToCells(GlyphSet::Braille)],
        );
        assert_eq!(v.output(), OutputKind::Cells);
        assert!(v.stages_fit().is_ok());
    }

    #[test]
    fn a_stage_that_takes_the_wrong_type_is_rejected() {
        // ViewText already produces cells, so ToCells has nothing to convert.
        let v = view(
            GraphicsKind::ViewText,
            vec![Stage::ToCells(GlyphSet::Mixed)],
        );
        let err = v.stages_fit().unwrap_err();
        assert!(err.contains("stage 0"), "{err}");
    }

    #[test]
    fn a_stage_pair_round_trips_back_to_the_start() {
        let v = view(
            GraphicsKind::ViewText,
            vec![
                Stage::ToPixels {
                    font: "cp437.tres.ron".into(),
                    cell: [8, 16],
                },
                Stage::ToCells(GlyphSet::Mixed),
            ],
        );
        assert!(v.stages_fit().is_ok(), "the chain itself is well typed");
        assert_eq!(v.output(), OutputKind::Cells);
    }

    #[test]
    fn the_root_view_is_the_one_named_main() {
        let mut views = BTreeMap::new();
        views.insert("game".to_string(), view(GraphicsKind::View2D, vec![]));
        let g = Game {
            name: "x".into(),
            input: "input.tres.ron".into(),
            views: views.clone(),
        };
        assert!(g.root().is_none(), "no main, no root");

        views.insert(ROOT_VIEW.to_string(), view(GraphicsKind::View2D, vec![]));
        let g = Game {
            views,
            ..g.clone()
        };
        assert!(g.root().is_some());
    }

    #[test]
    fn game_roundtrips_through_ron() {
        let mut views = BTreeMap::new();
        views.insert(
            ROOT_VIEW.to_string(),
            view(GraphicsKind::View2D, vec![Stage::ToCells(GlyphSet::Mixed)]),
        );
        let g = Game {
            name: "dino-run".into(),
            input: "input.tres.ron".into(),
            views,
        };
        let text = ron::ser::to_string_pretty(&g, Default::default()).unwrap();
        assert_eq!(ron::from_str::<Game>(&text).unwrap(), g);
    }

    /// Optional fields stay out of a serialized view.
    #[test]
    fn a_view_without_stages_serializes_compactly() {
        let text = ron::to_string(&view(GraphicsKind::View2D, vec![])).unwrap();
        assert!(!text.contains("stages"), "{text}");
    }
}

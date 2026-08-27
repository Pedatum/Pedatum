//! `canvas.ron`: a 2D tree in surface units. UI lives here.
//!
//! A canvas has no camera and no transform. Nodes are placed by layout rule,
//! and a canvas is rendered by a view whose projection is `Screen` — one canvas
//! unit to one surface unit.

use serde::{Deserialize, Serialize};

use crate::TexSource;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    pub name: String,
    pub nodes: Vec<CanvasNode>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasNode {
    pub name: String,
    #[serde(default)]
    pub rect: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<CanvasSprite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_rect: Option<ColorRect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CanvasNode>,
}

/// Where a node sits. `fill` ignores `size` and takes the whole parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default)]
    pub offset: [i32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
    #[serde(default)]
    pub fill: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Anchor {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    /// Fractions of the parent this anchor sits at, as (x, y) from the top left.
    pub fn fractions(self) -> (f32, f32) {
        let x = match self {
            Self::TopLeft | Self::Left | Self::BottomLeft => 0.0,
            Self::Top | Self::Center | Self::Bottom => 0.5,
            Self::TopRight | Self::Right | Self::BottomRight => 1.0,
        };
        let y = match self {
            Self::TopLeft | Self::Top | Self::TopRight => 0.0,
            Self::Left | Self::Center | Self::Right => 0.5,
            Self::BottomLeft | Self::Bottom | Self::BottomRight => 1.0,
        };
        (x, y)
    }
}

/// An image or another view, drawn into this node's rect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasSprite {
    pub source: TexSource,
}

/// Text. `from` names a component field in a world, so a HUD reads live state
/// without a system pushing it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Text {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub literal: Option<String>,
    /// `{}` is replaced by the value `from` resolves to.
    #[serde(default = "brace")]
    pub format: String,
}

fn brace() -> String {
    "{}".to_string()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ColorRect {
    /// Linear sRGB with alpha.
    pub color: [f32; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_map_to_parent_fractions() {
        assert_eq!(Anchor::TopLeft.fractions(), (0.0, 0.0));
        assert_eq!(Anchor::Center.fractions(), (0.5, 0.5));
        assert_eq!(Anchor::BottomRight.fractions(), (1.0, 1.0));
        assert_eq!(Anchor::TopRight.fractions(), (1.0, 0.0));
        assert_eq!(Anchor::BottomLeft.fractions(), (0.0, 1.0));
    }

    #[test]
    fn a_node_with_only_a_name_parses() {
        let n: CanvasNode = ron::from_str(r#"( name: "x" )"#).unwrap();
        assert_eq!(n.rect, Rect::default());
        assert!(n.sprite.is_none() && n.text.is_none() && n.color_rect.is_none());
    }

    #[test]
    fn a_sprite_may_be_an_image_or_a_view() {
        let png: CanvasNode =
            ron::from_str(r#"( name: "bg", sprite: Some((source: Png("a.png"))) )"#).unwrap();
        assert_eq!(
            png.sprite.unwrap().source,
            TexSource::Png("a.png".to_string())
        );

        let view: CanvasNode =
            ron::from_str(r#"( name: "vp", sprite: Some((source: View("game"))) )"#).unwrap();
        assert_eq!(
            view.sprite.unwrap().source,
            TexSource::View("game".to_string())
        );
    }

    #[test]
    fn text_defaults_to_passing_the_value_through() {
        let t: Text = ron::from_str(r#"( from: Some("Run.crashes") )"#).unwrap();
        assert_eq!(t.format, "{}");
    }

    #[test]
    fn canvas_roundtrips_through_ron() {
        let c = Canvas {
            name: "hud".into(),
            nodes: vec![
                CanvasNode {
                    name: "viewport".into(),
                    rect: Rect {
                        fill: true,
                        ..Default::default()
                    },
                    sprite: Some(CanvasSprite {
                        source: TexSource::View("game".into()),
                    }),
                    ..Default::default()
                },
                CanvasNode {
                    name: "score".into(),
                    rect: Rect {
                        anchor: Anchor::TopRight,
                        offset: [-2, 1],
                        ..Default::default()
                    },
                    text: Some(Text {
                        from: Some("Run.crashes".into()),
                        literal: None,
                        format: "crashes: {}".into(),
                    }),
                    ..Default::default()
                },
            ],
        };
        let text = ron::ser::to_string_pretty(&c, Default::default()).unwrap();
        assert_eq!(ron::from_str::<Canvas>(&text).unwrap(), c);
    }

    #[test]
    fn an_empty_node_serializes_without_its_optional_fields() {
        let text = ron::to_string(&CanvasNode {
            name: "n".into(),
            ..Default::default()
        })
        .unwrap();
        for field in ["sprite:", "text:", "color_rect:", "children:"] {
            assert!(!text.contains(field), "{field} should be skipped: {text}");
        }
    }
}

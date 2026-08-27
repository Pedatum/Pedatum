//! Cameras. A camera belongs to a View, never to a render unit: where a world
//! is seen from is the view's business, and the player's.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub projection: Projection,
    /// Absolute placement when `None`. Anchored, a camera reads a node in the
    /// target render unit without being one — a mirror on a car, a camera
    /// behind a player.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Anchor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Projection {
    Perspective {
        fov_y_degrees: f32,
        znear: f32,
        zfar: f32,
        /// Absolute placement, ignored when the camera is anchored.
        #[serde(default)]
        eye: [f32; 3],
        #[serde(default)]
        target: [f32; 3],
        #[serde(default = "up_y")]
        up: [f32; 3],
    },
    Orthographic {
        half_height: f32,
        znear: f32,
        zfar: f32,
        #[serde(default)]
        eye: [f32; 3],
        #[serde(default)]
        target: [f32; 3],
        #[serde(default = "up_y")]
        up: [f32; 3],
    },
    /// One canvas unit to one surface unit, no transform. What a canvas is
    /// rendered with.
    Screen,
    /// A cell grid, scrolled by whole cells. What `ViewText` is rendered with.
    CellViewport {
        origin: [i32; 2],
        #[serde(default)]
        cells_per_unit: [f32; 2],
    },
}

fn up_y() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}

/// Read a node's placement, and look from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    /// A node in the render unit the view targets.
    pub node: String,
    #[serde(default)]
    pub offset: [f32; 3],
    #[serde(default)]
    pub facing: Facing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Facing {
    /// The way the anchored node faces.
    #[default]
    Forward,
    /// The opposite way — a rear-view mirror.
    Backward,
    /// At a fixed point, wherever the node goes.
    Target([f32; 3]),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unanchored_camera_serializes_without_the_field() {
        let c = Camera {
            projection: Projection::Screen,
            anchor: None,
        };
        let text = ron::to_string(&c).unwrap();
        assert!(!text.contains("anchor"), "{text}");
    }

    #[test]
    fn a_perspective_camera_defaults_up_to_y() {
        let p: Projection =
            ron::from_str("Perspective(fov_y_degrees: 60.0, znear: 0.1, zfar: 100.0)").unwrap();
        match p {
            Projection::Perspective { up, .. } => assert_eq!(up, [0.0, 1.0, 0.0]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_rear_view_mirror_anchors_backward() {
        let c: Camera = ron::from_str(
            r#"(
                projection: Perspective(fov_y_degrees: 60.0, znear: 0.1, zfar: 100.0),
                anchor: Some(( node: "rear_mirror", facing: Backward )),
            )"#,
        )
        .unwrap();
        let a = c.anchor.unwrap();
        assert_eq!(a.node, "rear_mirror");
        assert_eq!(a.facing, Facing::Backward);
        assert_eq!(a.offset, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn camera_roundtrips_through_ron() {
        let c = Camera {
            projection: Projection::CellViewport {
                origin: [0, 0],
                cells_per_unit: [6.0, 3.0],
            },
            anchor: Some(Anchor {
                node: "player".into(),
                offset: [0.0, 2.0, 8.0],
                facing: Facing::Target([0.0, 1.0, 0.0]),
            }),
        };
        let text = ron::ser::to_string_pretty(&c, Default::default()).unwrap();
        assert_eq!(ron::from_str::<Camera>(&text).unwrap(), c);
    }
}

//! The world format: entities, their transforms, visuals and colliders.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::TexSource;

/// A world: entities in space. No camera — where it is seen from belongs to the
/// view that draws it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub nodes: Vec<Node>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<MeshRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<Sprite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tilemap: Option<Tilemap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collider: Option<Collider>,
    /// Game components, keyed by type name. The engine stores them opaquely;
    /// a game's compiled module deserializes each value into its own type.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<String, ron::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub translation: [f32; 3],
    /// Quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

/// Reference to a mesh asset (e.g., `assets/obj/<name>.obj`). Path is
/// workspace-relative — runtime resolves it the same way the existing
/// `Mesh::from_obj_file(path)` already does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshRef {
    pub path: String,
}

/// A textured quad cut from a sprite-sheet image by grid cell. The quad
/// faces +Z (side-view / 2D games) and is centered on the node's transform.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sprite {
    /// An image, or another view's output.
    pub source: TexSource,
    /// Sheet layout as [columns, rows], e.g. [2, 2].
    pub grid: [u32; 2],
    /// Cell to cut as [column, row]; [0, 0] is the top-left.
    pub cell: [u32; 2],
    /// World-space quad size [width, height].
    pub size: [f32; 2],
}

/// 2D tilemap. `tileset` is a path to a `.tres.ron` Tileset file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tilemap {
    pub tileset: String,
    pub tile_size: [f32; 2], // world-space size of one tile (XZ plane)
    pub cells: Vec<Cell>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
    pub tile_id: u32,
}

/// Collider shape. Both are axis-aligned: a rotated collider would need SAT
/// for boxes and a full conic test for ellipses, which is a larger change than
/// any current game needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    #[default]
    Box,
    /// Inscribed in `size`, so `size[0] / 2` is the x radius.
    Ellipse,
}

/// Collision volume, in world units. Authored, never derived from a visual, so
/// a scene plays identically under every View.
///
/// `size` means the same thing for every shape — the bounding box — so a
/// broadphase can compare bounds without knowing the shape.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider {
    #[serde(default)]
    pub shape: Shape,
    /// Full extents of the bounding box.
    pub size: [f32; 2],
    /// Centre relative to the node's translation. A character's box usually
    /// sits at its feet rather than its origin.
    #[serde(default)]
    pub offset: [f32; 2],
}

impl Scene {
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let pretty = ron::ser::PrettyConfig::default().depth_limit(8);
        let s = ron::ser::to_string_pretty(self, pretty)?;
        std::fs::write(path, s)?;
        Ok(())
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let scene: Scene = ron::from_str(&raw)?;
        Ok(scene)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Build an opaque game-component value the way a scene file carries it.
    fn comp(ron_src: &str) -> ron::Value {
        ron::from_str(ron_src).expect("component value should parse")
    }

    fn png(path: &str) -> TexSource {
        TexSource::Png(path.to_string())
    }

    fn sample_scene() -> Scene {
        Scene {
            name: "town".into(),
            nodes: vec![
                Node {
                    name: "ground".into(),
                    tilemap: Some(Tilemap {
                        tileset: "tilesets/town.tres.ron".into(),
                        tile_size: [1.0, 1.0],
                        cells: vec![
                            Cell { x: 0, y: 0, tile_id: 1 },
                            Cell { x: 1, y: 0, tile_id: 1 },
                            Cell { x: 2, y: 0, tile_id: 5 },
                        ],
                    }),
                    ..Default::default()
                },
                Node {
                    name: "prop".into(),
                    transform: Transform {
                        translation: [3.0, 0.0, 2.0],
                        ..Default::default()
                    },
                    mesh: Some(MeshRef {
                        path: "assets/obj/prop.obj".into(),
                    }),
                    components: BTreeMap::from([(
                        "SomeGameComponent".into(),
                        comp("(a: -32.0, b: 10.5)"),
                    )]),
                    ..Default::default()
                },
            ],
        }
    }

    #[test]
    fn scene_roundtrip() {
        let s1 = sample_scene();
        let text = ron::ser::to_string_pretty(&s1, Default::default()).unwrap();
        assert_eq!(ron::from_str::<Scene>(&text).unwrap(), s1);
    }

    #[test]
    fn empty_scene_roundtrip() {
        let s1 = Scene::default();
        let text = ron::to_string(&s1).unwrap();
        assert_eq!(ron::from_str::<Scene>(&text).unwrap(), s1);
    }

    /// A world holds no camera: that belongs to the view that draws it.
    #[test]
    fn a_world_has_no_camera_field() {
        let text = ron::to_string(&sample_scene()).unwrap();
        assert!(!text.contains("camera"), "{text}");
    }

    #[test]
    fn sprite_and_collider_roundtrip() {
        let n = Node {
            name: "actor".into(),
            sprite: Some(Sprite {
                source: png("assets/images/2x2_grid.png"),
                grid: [2, 2],
                cell: [0, 0],
                size: [1.2, 1.2],
            }),
            collider: Some(Collider {
                shape: Shape::Box,
                size: [0.84, 0.84],
                offset: [0.0, 0.0],
            }),
            ..Default::default()
        };
        let text = ron::ser::to_string_pretty(&n, Default::default()).unwrap();
        assert_eq!(ron::from_str::<Node>(&text).unwrap(), n);
    }

    /// A world object may show another view — a mirror, a monitor.
    #[test]
    fn a_sprite_may_be_another_views_output() {
        let n: Node = ron::from_str(
            r#"( name: "rear_mirror",
                 sprite: Some(( source: View("mirror"), grid: (1, 1), cell: (0, 0), size: (0.6, 0.2) )) )"#,
        )
        .unwrap();
        assert_eq!(
            n.sprite.unwrap().source,
            TexSource::View("mirror".to_string())
        );
    }

    /// Game components are opaque: any type name with any payload round-trips
    /// without the format knowing the type.
    #[test]
    fn unknown_game_components_roundtrip() {
        let n = Node {
            name: "actor".into(),
            components: BTreeMap::from([
                ("SomeGameComponent".into(), comp("(a: -32.0, b: 10.5)")),
                ("AnotherComponent".into(), comp("()")),
                ("ThirdComponent".into(), comp("(speed: -3.0, wrap: -7.0)")),
                ("SomeFutureComponent".into(), comp("(anything: [1, 2, 3])")),
            ]),
            ..Default::default()
        };
        let text = ron::ser::to_string_pretty(&n, Default::default()).unwrap();
        assert_eq!(ron::from_str::<Node>(&text).unwrap(), n);
    }

    /// A component the format has never heard of is not a parse error.
    #[test]
    fn scene_with_unregistered_component_still_parses() {
        let src = r#"(
            name: "x",
            nodes: [ ( name: "n", components: { "Whatever": (a: 1, b: "two") } ) ],
        )"#;
        let sc: Scene = ron::from_str(src).unwrap();
        assert!(sc.nodes[0].components.contains_key("Whatever"));
    }

    #[test]
    fn node_with_no_optional_fields_serializes_compactly() {
        let text = ron::to_string(&Node {
            name: "empty".into(),
            ..Default::default()
        })
        .unwrap();
        for field in ["mesh:", "sprite:", "tilemap:", "collider:", "components:", "children:"] {
            assert!(!text.contains(field), "{field} should be skipped: {text}");
        }
    }

    /// A collider authored before `shape` and `offset` existed still loads.
    #[test]
    fn collider_without_shape_or_offset_defaults_to_a_centred_box() {
        let c: Collider = ron::from_str("(size: (1.2, 1.2))").unwrap();
        assert_eq!(c.shape, Shape::Box);
        assert_eq!(c.offset, [0.0, 0.0]);
    }

    #[test]
    fn ellipse_collider_roundtrips() {
        let c = Collider {
            shape: Shape::Ellipse,
            size: [2.0, 1.0],
            offset: [0.0, -0.5],
        };
        let text = ron::to_string(&c).unwrap();
        assert_eq!(ron::from_str::<Collider>(&text).unwrap(), c);
    }

    #[test]
    fn save_load_roundtrip() {
        let s1 = sample_scene();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        s1.save(tmp.path()).unwrap();
        assert_eq!(Scene::load(tmp.path()).unwrap(), s1);
    }

    #[test]
    fn load_nonexistent_returns_error() {
        assert!(Scene::load(Path::new("/tmp/nonexistent_scene_12345.ron")).is_err());
    }
}

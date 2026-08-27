use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Top-level type stored in `.scn.ron` or `scene.ron`. The optional `camera`
/// is the game camera the player would see; the editor viewport uses its
/// own camera and ignores this field.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Camera>,
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
    /// Path to the sheet image (e.g. `assets/images/2x2_grid.png`),
    /// workspace-relative like mesh paths.
    pub sheet: String,
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

/// Top-level type stored in `.tres.ron` for tilesets.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tileset {
    pub name: String,
    pub tiles: Vec<Tile>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub id: u32,
    pub name: String,
    /// Linear sRGB color [r, g, b], each in 0.0..=1.0. v1 has no texture
    /// support; v2 can add `atlas: Option<String>` + `uv: Option<[u32; 4]>`
    /// without breaking existing files.
    pub color: [f32; 3],
}

/// Collision volume, in world units. Authored, never derived from a visual, so
/// a scene plays identically under every View.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider {
    /// Full extents, centered on the node's translation.
    pub size: [f32; 2],
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

/// Top-level type stored in `tscn.ron` — describes the camera the editor /
/// runner should use when rendering a scene. Fields mirror the engine's
/// `Camera`/`Projection` but are serde-friendly (f32 arrays, degrees).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub projection: Projection,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Projection {
    Perspective {
        fov_y_degrees: f32,
        aspect: f32,
        znear: f32,
        zfar: f32,
    },
    Orthographic {
        half_height: f32,
        aspect: f32,
        znear: f32,
        zfar: f32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an opaque game-component value the way a scene file would carry it.
    fn comp(ron_src: &str) -> ron::Value {
        ron::from_str(ron_src).expect("component value should parse")
    }

    fn sample_scene() -> Scene {
        Scene {
            name: "town".into(),
            camera: None,
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

    fn sample_tileset() -> Tileset {
        Tileset {
            name: "town_tiles".into(),
            tiles: vec![
                Tile { id: 1, name: "grass".into(), color: [0.4, 0.8, 0.3] },
                Tile { id: 2, name: "river".into(), color: [0.2, 0.5, 0.9] },
                Tile { id: 5, name: "path".into(),  color: [0.7, 0.6, 0.4] },
            ],
        }
    }

    #[test]
    fn scene_roundtrip() {
        let s1 = sample_scene();
        let serialized = ron::ser::to_string_pretty(&s1, Default::default()).unwrap();
        let s2: Scene = ron::from_str(&serialized).unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn tileset_roundtrip() {
        let t1 = sample_tileset();
        let serialized = ron::ser::to_string_pretty(&t1, Default::default()).unwrap();
        let t2: Tileset = ron::from_str(&serialized).unwrap();
        assert_eq!(t1, t2);
    }

    #[test]
    fn empty_scene_roundtrip() {
        let s1 = Scene::default();
        let serialized = ron::to_string(&s1).unwrap();
        let s2: Scene = ron::from_str(&serialized).unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn sprite_and_collider_roundtrip() {
        let n = Node {
            name: "actor".into(),
            sprite: Some(Sprite {
                sheet: "assets/images/2x2_grid.png".into(),
                grid: [2, 2],
                cell: [0, 0],
                size: [1.2, 1.2],
            }),
            collider: Some(Collider { size: [0.84, 0.84] }),
            ..Default::default()
        };
        let s = ron::ser::to_string_pretty(&n, Default::default()).unwrap();
        let n2: Node = ron::from_str(&s).unwrap();
        assert_eq!(n, n2);
    }

    /// Game components are opaque to the engine: any type name with any payload
    /// round-trips without the scene crate knowing the type.
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
        let s = ron::ser::to_string_pretty(&n, Default::default()).unwrap();
        let n2: Node = ron::from_str(&s).unwrap();
        assert_eq!(n, n2);
    }

    /// A component the engine has never heard of must not be a parse error —
    /// resolution is the game module's job, at registry time.
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
        let n = Node { name: "empty".into(), ..Default::default() };
        let s = ron::to_string(&n).unwrap();
        for field in ["mesh:", "sprite:", "tilemap:", "collider:", "components:", "children:"] {
            assert!(!s.contains(field), "{field} should be skipped: {s}");
        }
    }

    #[test]
    fn save_load_roundtrip() {
        let s1 = sample_scene();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        s1.save(tmp.path()).unwrap();
        let s2 = Scene::load(tmp.path()).unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn save_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_scene.scn.ron");
        sample_scene().save(&path).unwrap();
        assert!(path.exists());
        let raw = std::fs::read_to_string(&path).unwrap();
        let _: Scene = ron::from_str(&raw).unwrap();
    }

    #[test]
    fn load_nonexistent_returns_error() {
        assert!(Scene::load(Path::new("/tmp/nonexistent_scene_12345.ron")).is_err());
    }
}

//! Tilesets, stored in `.tres.ron`.

use serde::{Deserialize, Serialize};

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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tileset_roundtrip() {
        let t = Tileset {
            name: "town_tiles".into(),
            tiles: vec![
                Tile { id: 1, name: "grass".into(), color: [0.4, 0.8, 0.3] },
                Tile { id: 2, name: "river".into(), color: [0.2, 0.5, 0.9] },
                Tile { id: 5, name: "path".into(),  color: [0.7, 0.6, 0.4] },
            ],
        };
        let text = ron::ser::to_string_pretty(&t, Default::default()).unwrap();
        assert_eq!(ron::from_str::<Tileset>(&text).unwrap(), t);
    }
}

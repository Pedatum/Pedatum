use scene::{Node, Transform};

const FIELD_COUNT: usize = 10;

pub struct InspectorState {
    pub node_name: String,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub mesh_path: Option<String>,
    pub editing: bool,
    pub field_cursor: usize,
}

impl InspectorState {
    pub fn from_node(node: &Node) -> Self {
        Self {
            node_name: node.name.clone(),
            translation: node.transform.translation,
            rotation: node.transform.rotation,
            scale: node.transform.scale,
            mesh_path: node.mesh.as_ref().map(|m| m.path.clone()),
            editing: false,
            field_cursor: 0,
        }
    }

    pub fn clear() -> Self {
        Self {
            node_name: String::new(),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            mesh_path: None,
            editing: false,
            field_cursor: 0,
        }
    }

    pub fn enter_edit(&mut self) {
        self.editing = true;
    }

    pub fn exit_edit(&mut self) {
        self.editing = false;
    }

    pub fn next_field(&mut self) {
        self.field_cursor = (self.field_cursor + 1) % FIELD_COUNT;
    }

    pub fn adjust(&mut self, delta: f32) {
        match self.field_cursor {
            0..3 => self.translation[self.field_cursor] += delta,
            3..7 => self.rotation[self.field_cursor - 3] += delta,
            7..10 => self.scale[self.field_cursor - 7] += delta,
            _ => {}
        }
    }

    pub fn to_transform(&self) -> Transform {
        Transform {
            translation: self.translation,
            rotation: self.rotation,
            scale: self.scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene::MeshRef;

    fn sample_node() -> Node {
        Node {
            name: "player".into(),
            transform: Transform {
                translation: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 0.7071, 0.7071],
                scale: [2.0, 2.0, 2.0],
            },
            mesh: Some(MeshRef {
                path: "assets/player.obj".into(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn from_node_copies_fields() {
        let node = sample_node();
        let state = InspectorState::from_node(&node);
        assert_eq!(state.node_name, "player");
        assert_eq!(state.translation, [1.0, 2.0, 3.0]);
        assert_eq!(state.rotation, [0.0, 0.0, 0.7071, 0.7071]);
        assert_eq!(state.scale, [2.0, 2.0, 2.0]);
        assert_eq!(state.mesh_path.as_deref(), Some("assets/player.obj"));
        assert!(!state.editing);
        assert_eq!(state.field_cursor, 0);
    }

    #[test]
    fn clear_returns_defaults() {
        let state = InspectorState::clear();
        assert!(state.node_name.is_empty());
        assert_eq!(state.translation, [0.0, 0.0, 0.0]);
        assert_eq!(state.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(state.scale, [1.0, 1.0, 1.0]);
        assert!(state.mesh_path.is_none());
    }

    #[test]
    fn edit_mode_toggle() {
        let mut state = InspectorState::clear();
        assert!(!state.editing);
        state.enter_edit();
        assert!(state.editing);
        state.exit_edit();
        assert!(!state.editing);
    }

    #[test]
    fn next_field_wraps() {
        let mut state = InspectorState::clear();
        for _ in 0..9 {
            state.next_field();
        }
        assert_eq!(state.field_cursor, 9);
        state.next_field();
        assert_eq!(state.field_cursor, 0);
    }

    #[test]
    fn adjust_translation() {
        let mut state = InspectorState::clear();
        state.adjust(1.5);
        assert_eq!(state.translation, [1.5, 0.0, 0.0]);
        state.next_field();
        state.adjust(-0.5);
        assert_eq!(state.translation, [1.5, -0.5, 0.0]);
    }

    #[test]
    fn adjust_rotation() {
        let mut state = InspectorState::clear();
        state.field_cursor = 3;
        state.adjust(0.1);
        assert_eq!(state.rotation[0], 0.1);
    }

    #[test]
    fn adjust_scale() {
        let mut state = InspectorState::clear();
        state.field_cursor = 7;
        state.adjust(0.5);
        assert_eq!(state.scale, [1.5, 1.0, 1.0]);
    }

    #[test]
    fn to_transform_roundtrip() {
        let node = sample_node();
        let state = InspectorState::from_node(&node);
        let t = state.to_transform();
        assert_eq!(t.translation, node.transform.translation);
        assert_eq!(t.rotation, node.transform.rotation);
        assert_eq!(t.scale, node.transform.scale);
    }

    #[test]
    fn from_node_without_mesh() {
        let node = Node {
            name: "empty".into(),
            ..Default::default()
        };
        let state = InspectorState::from_node(&node);
        assert!(state.mesh_path.is_none());
    }
}

use scene::Scene;

pub struct HierarchyEntry {
    pub name: String,
    pub depth: usize,
    pub expanded: bool,
    pub has_children: bool,
    pub node_index: usize,
}

pub struct HierarchyState {
    pub entries: Vec<HierarchyEntry>,
    pub selected: usize,
}

impl HierarchyState {
    pub fn from_scene(scene: &Scene) -> Self {
        let mut entries = Vec::new();
        for (i, node) in scene.nodes.iter().enumerate() {
            flatten_node(node, i, 0, &mut entries);
        }
        Self {
            entries,
            selected: 0,
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    pub fn toggle_expand(&mut self) {
        if let Some(entry) = self.entries.get_mut(self.selected) {
            if entry.has_children {
                entry.expanded = !entry.expanded;
            }
        }
    }

    pub fn selected_node_index(&self) -> Option<usize> {
        self.entries.get(self.selected).map(|e| e.node_index)
    }
}

fn flatten_node(
    node: &scene::Node,
    node_index: usize,
    depth: usize,
    entries: &mut Vec<HierarchyEntry>,
) {
    let has_children = !node.children.is_empty();
    entries.push(HierarchyEntry {
        name: node.name.clone(),
        depth,
        expanded: true,
        has_children,
        node_index,
    });
    for (i, child) in node.children.iter().enumerate() {
        flatten_node(child, node_index * 1000 + i, depth + 1, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene::{MeshRef, Node};

    fn scene_with_children() -> Scene {
        Scene {
            name: "test".into(),
            nodes: vec![
                Node {
                    name: "parent".into(),
                    children: vec![
                        Node {
                            name: "child_a".into(),
                            ..Default::default()
                        },
                        Node {
                            name: "child_b".into(),
                            children: vec![Node {
                                name: "grandchild".into(),
                                ..Default::default()
                            }],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                Node {
                    name: "sibling".into(),
                    mesh: Some(MeshRef {
                        path: "assets/cube.obj".into(),
                    }),
                    ..Default::default()
                },
            ],
        }
    }

    #[test]
    fn flatten_produces_correct_order_and_depth() {
        let scene = scene_with_children();
        let state = HierarchyState::from_scene(&scene);
        let names: Vec<&str> = state.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["parent", "child_a", "child_b", "grandchild", "sibling"]
        );
        let depths: Vec<usize> = state.entries.iter().map(|e| e.depth).collect();
        assert_eq!(depths, vec![0, 1, 1, 2, 0]);
    }

    #[test]
    fn has_children_flags() {
        let scene = scene_with_children();
        let state = HierarchyState::from_scene(&scene);
        let flags: Vec<bool> = state.entries.iter().map(|e| e.has_children).collect();
        assert_eq!(flags, vec![true, false, true, false, false]);
    }

    #[test]
    fn navigation() {
        let scene = scene_with_children();
        let mut state = HierarchyState::from_scene(&scene);
        assert_eq!(state.selected, 0);
        state.move_up();
        assert_eq!(state.selected, 0);
        state.move_down();
        assert_eq!(state.selected, 1);
        state.move_down();
        state.move_down();
        state.move_down();
        assert_eq!(state.selected, 4);
        state.move_down();
        assert_eq!(state.selected, 4);
    }

    #[test]
    fn toggle_expand() {
        let scene = scene_with_children();
        let mut state = HierarchyState::from_scene(&scene);
        assert!(state.entries[0].expanded);
        state.toggle_expand();
        assert!(!state.entries[0].expanded);
        state.toggle_expand();
        assert!(state.entries[0].expanded);
    }

    #[test]
    fn toggle_expand_noop_on_leaf() {
        let scene = scene_with_children();
        let mut state = HierarchyState::from_scene(&scene);
        state.move_down(); // child_a — leaf node
        state.toggle_expand();
        assert!(!state.entries[1].has_children);
        assert!(state.entries[1].expanded); // unchanged
    }

    #[test]
    fn selected_node_index() {
        let scene = scene_with_children();
        let state = HierarchyState::from_scene(&scene);
        assert_eq!(state.selected_node_index(), Some(0));
    }

    #[test]
    fn empty_scene() {
        let scene = Scene::default();
        let state = HierarchyState::from_scene(&scene);
        assert!(state.entries.is_empty());
        assert_eq!(state.selected_node_index(), None);
    }
}

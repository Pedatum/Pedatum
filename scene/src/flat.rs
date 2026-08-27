//! Flattening a world's node tree, and putting it back.
//!
//! RON carries a tree because that is how a scene is written. A runtime wants a
//! flat list, because an entity is one row and hierarchy is a component. These
//! two functions are the only place the two shapes meet.
//!
//! `parent` plus `sibling` is what makes the round trip exact: without a
//! sibling index the same tree saved twice could order children differently.

use serde::{Deserialize, Serialize};

use crate::{Node, Transform};

/// One node, its place in the tree recorded rather than nested.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlatNode {
    /// The node with its `children` emptied.
    pub node: Node,
    /// Index into the flat list, or `None` for a root.
    pub parent: Option<usize>,
    /// Position among its parent's children, so a save is stable.
    pub sibling: u32,
}

/// Depth-first, parents before children, siblings in order.
pub fn flatten(nodes: &[Node]) -> Vec<FlatNode> {
    let mut out = Vec::new();
    push_level(nodes, None, &mut out);
    out
}

fn push_level(nodes: &[Node], parent: Option<usize>, out: &mut Vec<FlatNode>) {
    for (sibling, node) in nodes.iter().enumerate() {
        let mut flat = node.clone();
        flat.children = Vec::new();
        let index = out.len();
        out.push(FlatNode {
            node: flat,
            parent,
            sibling: sibling as u32,
        });
        push_level(&node.children, Some(index), out);
    }
}

/// Rebuild the tree. Children come back in `sibling` order, and a node whose
/// parent index is out of range is treated as a root rather than dropped.
pub fn unflatten(flat: &[FlatNode]) -> Vec<Node> {
    let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); flat.len()];
    let mut roots: Vec<usize> = Vec::new();

    for (i, f) in flat.iter().enumerate() {
        match f.parent {
            Some(p) if p < flat.len() && p != i => children_of[p].push(i),
            _ => roots.push(i),
        }
    }
    for list in &mut children_of {
        list.sort_by_key(|i| flat[*i].sibling);
    }
    roots.sort_by_key(|i| flat[*i].sibling);

    roots
        .into_iter()
        .map(|i| build(i, flat, &children_of))
        .collect()
}

fn build(i: usize, flat: &[FlatNode], children_of: &[Vec<usize>]) -> Node {
    let mut node = flat[i].node.clone();
    node.children = children_of[i]
        .iter()
        .map(|c| build(*c, flat, children_of))
        .collect();
    node
}

/// A node's transform composed with every ancestor's, as translation and scale.
///
/// The engine's renderer needs this; a game's systems write the local
/// transform and never see it.
pub fn global_transform(flat: &[FlatNode], index: usize) -> Transform {
    let mut chain = Vec::new();
    let mut cursor = Some(index);
    while let Some(i) = cursor {
        chain.push(i);
        cursor = flat[i].parent.filter(|p| *p < flat.len() && *p != i);
        if chain.len() > flat.len() {
            break; // a cycle in the data; stop rather than spin
        }
    }

    let mut out = Transform::default();
    for i in chain.into_iter().rev() {
        let local = &flat[i].node.transform;
        for axis in 0..3 {
            out.translation[axis] += local.translation[axis] * out.scale[axis];
            out.scale[axis] *= local.scale[axis];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, children: Vec<Node>) -> Node {
        Node {
            name: name.into(),
            children,
            ..Default::default()
        }
    }

    fn at(name: &str, x: f32, scale: f32, children: Vec<Node>) -> Node {
        Node {
            name: name.into(),
            transform: Transform {
                translation: [x, 0.0, 0.0],
                scale: [scale, scale, scale],
                ..Default::default()
            },
            children,
            ..Default::default()
        }
    }

    fn tree() -> Vec<Node> {
        vec![
            node("a", vec![node("a1", vec![node("a1x", vec![])]), node("a2", vec![])]),
            node("b", vec![]),
        ]
    }

    #[test]
    fn flatten_is_depth_first_parents_before_children() {
        let flat = flatten(&tree());
        let names: Vec<&str> = flat.iter().map(|f| f.node.name.as_str()).collect();
        assert_eq!(names, ["a", "a1", "a1x", "a2", "b"]);
    }

    #[test]
    fn flatten_records_the_parent_and_the_sibling_slot() {
        let flat = flatten(&tree());
        assert_eq!(flat[0].parent, None, "a is a root");
        assert_eq!(flat[0].sibling, 0);
        assert_eq!(flat[1].parent, Some(0), "a1 under a");
        assert_eq!(flat[2].parent, Some(1), "a1x under a1");
        assert_eq!(flat[3].parent, Some(0));
        assert_eq!(flat[3].sibling, 1, "a2 is a's second child");
        assert_eq!(flat[4].parent, None, "b is a root");
        assert_eq!(flat[4].sibling, 1);
    }

    #[test]
    fn a_flattened_node_carries_no_children() {
        assert!(flatten(&tree()).iter().all(|f| f.node.children.is_empty()));
    }

    #[test]
    fn the_round_trip_is_exact() {
        let original = tree();
        assert_eq!(unflatten(&flatten(&original)), original);
    }

    /// The sibling index is what makes it exact: reversing the flat list must
    /// not reorder the tree.
    #[test]
    fn sibling_order_survives_a_reordered_flat_list() {
        let original = tree();
        let mut flat = flatten(&original);
        // Renumber parents for the reversal, then check the tree comes back.
        let n = flat.len();
        flat.reverse();
        for f in &mut flat {
            f.parent = f.parent.map(|p| n - 1 - p);
        }
        assert_eq!(unflatten(&flat), original);
    }

    #[test]
    fn an_empty_tree_round_trips() {
        assert_eq!(unflatten(&flatten(&[])), Vec::<Node>::new());
    }

    /// Corrupt data becomes a root rather than vanishing.
    #[test]
    fn a_node_pointing_past_the_list_becomes_a_root() {
        let mut flat = flatten(&vec![node("a", vec![node("a1", vec![])])]);
        flat[1].parent = Some(99);
        let rebuilt = unflatten(&flat);
        assert_eq!(rebuilt.len(), 2, "a1 comes back as a second root");
    }

    #[test]
    fn a_node_that_is_its_own_parent_becomes_a_root() {
        let mut flat = flatten(&vec![node("a", vec![])]);
        flat[0].parent = Some(0);
        assert_eq!(unflatten(&flat).len(), 1);
    }

    // ── global transform ────────────────────────────────────────────────────

    #[test]
    fn a_root_transform_is_its_own() {
        let flat = flatten(&vec![at("a", 3.0, 2.0, vec![])]);
        let g = global_transform(&flat, 0);
        assert_eq!(g.translation[0], 3.0);
        assert_eq!(g.scale[0], 2.0);
    }

    #[test]
    fn a_child_is_scaled_and_offset_by_its_parent() {
        // a at x=3 scale 2; a1 at local x=1 → world x = 3 + 1*2 = 5, scale 2*3.
        let flat = flatten(&vec![at("a", 3.0, 2.0, vec![at("a1", 1.0, 3.0, vec![])])]);
        let g = global_transform(&flat, 1);
        assert_eq!(g.translation[0], 5.0);
        assert_eq!(g.scale[0], 6.0);
    }

    #[test]
    fn transforms_compose_through_three_levels() {
        let flat = flatten(&vec![at(
            "a",
            1.0,
            2.0,
            vec![at("a1", 1.0, 2.0, vec![at("a1x", 1.0, 1.0, vec![])])],
        )]);
        // 1 + 1*2 = 3, then 3 + 1*4 = 7.
        assert_eq!(global_transform(&flat, 2).translation[0], 7.0);
        assert_eq!(global_transform(&flat, 2).scale[0], 4.0);
    }

    /// A cycle in the data must not spin forever.
    #[test]
    fn a_parent_cycle_terminates() {
        let mut flat = flatten(&vec![at("a", 1.0, 1.0, vec![at("a1", 1.0, 1.0, vec![])])]);
        flat[0].parent = Some(1);
        let _ = global_transform(&flat, 1);
    }
}

//! Running mode: plays a scene like the real game runner. Pure data logic on
//! a cloned `scene::Scene` (no GPU) — the viewport just renders the mutated
//! copy each frame. Behavior comes from `ComponentValue` tags:
//! `PlayerControlled` (gravity + space-to-jump), `ScrollX` (auto-scroll with
//! wrap-around), `Obstacle` (collision with the player resets the run), and
//! `Dialogue` (visual-novel lines advanced with Space).

use scene::{ComponentValue, DialogueLine, Node, Scene};

pub const GRAVITY: f32 = -32.0;
pub const JUMP_VELOCITY: f32 = 10.5;
/// Half-extent of a collision box as a fraction of sprite size — forgiving
/// hitboxes, matching the padding inside the sprite-sheet cards.
const HITBOX_HALF: f32 = 0.35;
/// Clamp a frame's dt so a stalled draw can't tunnel through obstacles.
const MAX_DT: f32 = 0.1;

pub struct RunState {
    /// The live, mutated copy the viewport renders.
    pub scene: Scene,
    initial: Scene,
    pub vy: f32,
    jump_queued: bool,
    pub elapsed: f32,
    pub crashes: u32,
    dialogue_index: usize,
}

impl RunState {
    pub fn new(scene: Scene) -> Self {
        Self {
            initial: scene.clone(),
            scene,
            vy: 0.0,
            jump_queued: false,
            elapsed: 0.0,
            crashes: 0,
            dialogue_index: 0,
        }
    }

    pub fn queue_jump(&mut self) {
        self.jump_queued = true;
    }

    /// Whether this scene owns the Space key for dialogue progression.
    pub fn has_dialogue(&self) -> bool {
        dialogue_lines(&self.scene).is_some()
    }

    pub fn current_dialogue(&self) -> Option<&DialogueLine> {
        dialogue_lines(&self.scene)?.get(self.dialogue_index)
    }

    pub fn dialogue_progress(&self) -> Option<(usize, usize)> {
        let lines = dialogue_lines(&self.scene)?;
        Some((self.dialogue_index.min(lines.len()), lines.len()))
    }

    /// Advance to the next line. After the last line, `current_dialogue`
    /// becomes `None` so the UI can show an end marker.
    pub fn advance_dialogue(&mut self) {
        if let Some(lines) = dialogue_lines(&self.scene) {
            self.dialogue_index = (self.dialogue_index + 1).min(lines.len());
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, MAX_DT);
        self.elapsed += dt;

        // ScrollX system.
        for node in &mut self.scene.nodes {
            let scroll = node.components.iter().find_map(|c| match c {
                ComponentValue::ScrollX {
                    speed,
                    wrap_at,
                    reset_to,
                } => Some((*speed, *wrap_at, *reset_to)),
                _ => None,
            });
            if let Some((speed, wrap_at, reset_to)) = scroll {
                let x = &mut node.transform.translation[0];
                *x += speed * dt;
                if (speed < 0.0 && *x < wrap_at) || (speed > 0.0 && *x > wrap_at) {
                    *x = reset_to;
                }
            }
        }

        // Player: jump impulse + gravity. Ground level = the player's
        // starting y, so scenes choose their own floor.
        let jump = std::mem::take(&mut self.jump_queued);
        let Some(idx) = player_index(&self.scene) else {
            return;
        };
        let ground = self.initial.nodes[idx].transform.translation[1];
        {
            let y = &mut self.scene.nodes[idx].transform.translation[1];
            let on_ground = *y <= ground + 1e-3;
            if jump && on_ground {
                self.vy = JUMP_VELOCITY;
            }
            self.vy += GRAVITY * dt;
            *y += self.vy * dt;
            if *y <= ground {
                *y = ground;
                self.vy = 0.0;
            }
        }

        // Collision: player AABB vs every Obstacle → restart the run.
        let player_rect = rect(&self.scene.nodes[idx]);
        let hit = self.scene.nodes.iter().enumerate().any(|(i, n)| {
            i != idx
                && n.components
                    .iter()
                    .any(|c| matches!(c, ComponentValue::Obstacle))
                && overlaps(player_rect, rect(n))
        });
        if hit {
            self.crashes += 1;
            self.scene = self.initial.clone();
            self.vy = 0.0;
            self.elapsed = 0.0;
            self.dialogue_index = 0;
        }
    }
}

fn dialogue_lines(nodes_scene: &Scene) -> Option<&[DialogueLine]> {
    fn find(nodes: &[Node]) -> Option<&[DialogueLine]> {
        for node in nodes {
            if let Some(lines) = node
                .components
                .iter()
                .find_map(|component| match component {
                    ComponentValue::Dialogue { lines } => Some(lines.as_slice()),
                    _ => None,
                })
            {
                return Some(lines);
            }
            if let Some(lines) = find(&node.children) {
                return Some(lines);
            }
        }
        None
    }

    find(&nodes_scene.nodes)
}

pub fn player_index(scene: &Scene) -> Option<usize> {
    scene.nodes.iter().position(|n| {
        n.components
            .iter()
            .any(|c| matches!(c, ComponentValue::PlayerControlled))
    })
}

/// XY collision box: (center, half extents). Sized from the node's sprite;
/// nodes without one get a 1x1 box.
fn rect(node: &Node) -> ([f32; 2], [f32; 2]) {
    let t = node.transform.translation;
    let size = node.sprite.as_ref().map(|s| s.size).unwrap_or([1.0, 1.0]);
    ([t[0], t[1]], [size[0] * HITBOX_HALF, size[1] * HITBOX_HALF])
}

fn overlaps(a: ([f32; 2], [f32; 2]), b: ([f32; 2], [f32; 2])) -> bool {
    (a.0[0] - b.0[0]).abs() < a.1[0] + b.1[0] && (a.0[1] - b.0[1]).abs() < a.1[1] + b.1[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene::{Sprite, Transform};

    fn sprite(size: [f32; 2]) -> Option<Sprite> {
        Some(Sprite {
            sheet: "assets/images/2x2_grid.png".into(),
            grid: [2, 2],
            cell: [0, 0],
            size,
        })
    }

    fn node(name: &str, x: f32, y: f32, components: Vec<ComponentValue>) -> Node {
        Node {
            name: name.into(),
            transform: Transform {
                translation: [x, y, 0.0],
                ..Default::default()
            },
            sprite: sprite([1.0, 1.0]),
            components,
            ..Default::default()
        }
    }

    fn run_scene(nodes: Vec<Node>) -> RunState {
        RunState::new(Scene {
            name: "test".into(),
            camera: None,
            nodes,
        })
    }

    #[test]
    fn jump_rises_then_lands_back_on_ground() {
        let mut run = run_scene(vec![node(
            "dino",
            0.0,
            0.0,
            vec![ComponentValue::PlayerControlled],
        )]);
        run.queue_jump();
        run.tick(0.05);
        let y_up = run.scene.nodes[0].transform.translation[1];
        assert!(y_up > 0.0, "player should rise after jump, y={y_up}");

        for _ in 0..40 {
            run.tick(0.05);
        }
        assert_eq!(run.scene.nodes[0].transform.translation[1], 0.0);
        assert_eq!(run.vy, 0.0);
    }

    #[test]
    fn no_double_jump_midair() {
        let mut run = run_scene(vec![node(
            "dino",
            0.0,
            0.0,
            vec![ComponentValue::PlayerControlled],
        )]);
        run.queue_jump();
        run.tick(0.05);
        let vy_after_first = run.vy;
        run.queue_jump();
        run.tick(0.05);
        assert!(
            run.vy < vy_after_first,
            "midair jump must not re-apply impulse"
        );
    }

    #[test]
    fn scroller_wraps_to_reset_position() {
        let mut run = run_scene(vec![node(
            "tree",
            -0.95,
            0.0,
            vec![ComponentValue::ScrollX {
                speed: -1.0,
                wrap_at: -1.0,
                reset_to: 5.0,
            }],
        )]);
        run.tick(0.1);
        assert_eq!(run.scene.nodes[0].transform.translation[0], 5.0);
    }

    #[test]
    fn obstacle_collision_resets_run() {
        let mut run = run_scene(vec![
            node("dino", 0.0, 0.0, vec![ComponentValue::PlayerControlled]),
            node(
                "tree",
                3.0,
                0.0,
                vec![
                    ComponentValue::Obstacle,
                    ComponentValue::ScrollX {
                        speed: -30.0,
                        wrap_at: -10.0,
                        reset_to: 10.0,
                    },
                ],
            ),
        ]);
        // Tree scrolls into the player within ~1s of clamped ticks.
        for _ in 0..12 {
            run.tick(0.1);
        }
        assert!(run.crashes >= 1, "tree should have hit the player");
        assert_eq!(run.scene.nodes[1].transform.translation[0], 3.0);
        assert_eq!(run.elapsed, 0.0);
    }

    #[test]
    fn scenery_without_obstacle_does_not_reset() {
        let mut run = run_scene(vec![
            node("dino", 0.0, 0.0, vec![ComponentValue::PlayerControlled]),
            node(
                "cloud",
                0.0,
                0.0, // overlapping the player the whole time
                vec![ComponentValue::ScrollX {
                    speed: -0.1,
                    wrap_at: -10.0,
                    reset_to: 10.0,
                }],
            ),
        ]);
        run.tick(0.1);
        assert_eq!(run.crashes, 0);
    }

    #[test]
    fn elapsed_accumulates_and_dt_is_clamped() {
        let mut run = run_scene(vec![node(
            "dino",
            0.0,
            0.0,
            vec![ComponentValue::PlayerControlled],
        )]);
        run.tick(5.0); // clamped to MAX_DT
        assert!(run.elapsed <= MAX_DT + 1e-6);
    }

    #[test]
    fn dialogue_advances_one_line_per_action() {
        let story = node(
            "story",
            0.0,
            0.0,
            vec![ComponentValue::Dialogue {
                lines: vec![
                    DialogueLine {
                        speaker: "Narrator".into(),
                        text: "First".into(),
                    },
                    DialogueLine {
                        speaker: "Mina".into(),
                        text: "Second".into(),
                    },
                ],
            }],
        );
        let mut run = run_scene(vec![story]);

        assert!(run.has_dialogue());
        assert_eq!(run.current_dialogue().unwrap().text, "First");
        assert_eq!(run.dialogue_progress(), Some((0, 2)));

        run.advance_dialogue();
        assert_eq!(run.current_dialogue().unwrap().text, "Second");
        assert_eq!(run.dialogue_progress(), Some((1, 2)));

        run.advance_dialogue();
        assert!(run.current_dialogue().is_none());
        assert_eq!(run.dialogue_progress(), Some((2, 2)));
    }
}

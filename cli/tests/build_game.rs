//! End-to-end: `shinra build` on a game folder that contains only data and
//! `.hom`, then load the module it produced and check the systems actually run.
//!
//! This is the proof that game logic lives in the game. Run
//! `cargo run -p shinra-cli -- build ../shinra-examples/assets/games/game3`
//! first; the test skips itself if the module is not there.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use gametok_abi::keys;
use shinra_engine::registry::GameModule;

/// A game module is single-instance per process, so these tests take turns.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn examples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../shinra-examples")
        .canonicalize()
        .expect("shinra-examples should sit beside shinra-engine")
}

fn so_path() -> PathBuf {
    examples().join("target/games/libgame3.so")
}

/// True when `shinra build` has not been run for game3 yet.
fn missing() -> bool {
    if so_path().exists() {
        return false;
    }
    eprintln!(
        "skipping: {} not built. Run: cargo run -p shinra-cli -- build ../shinra-examples/assets/games/game3",
        so_path().display()
    );
    true
}

fn scene_ron() -> String {
    std::fs::read_to_string(examples().join("assets/games/game3/world.ron"))
        .expect("read game3 world")
}

fn load() -> GameModule {
    GameModule::load(&so_path(), &scene_ron()).expect("load libgame3.so")
}

fn node_y(scene: &scene::Scene, name: &str) -> f32 {
    scene
        .nodes
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node {name}"))
        .transform
        .translation[1]
}

fn node_x(scene: &scene::Scene, name: &str) -> f32 {
    scene
        .nodes
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node {name}"))
        .transform
        .translation[0]
}

#[test]
fn module_loads_and_returns_its_scene() {
    let _s = serial();
    if missing() { return }
    let game = load();
    let scene = game.scene().expect("scene round-trip");
    assert_eq!(scene.name, "dino-run");
    assert_eq!(node_y(&scene, "dino"), 0.0);
}

/// player.hom: the "jump" action gives the dino upward velocity, gravity brings
/// it back to its ground line.
#[test]
fn jump_action_lifts_the_dino_then_gravity_lands_it() {
    let _s = serial();
    if missing() { return }
    let game = load();
    let ground = node_y(&game.scene().unwrap(), "dino");

    game.tick(0.016, &[keys::SPACE], &[keys::SPACE]).unwrap();
    let after_jump = node_y(&game.scene().unwrap(), "dino");
    assert!(
        after_jump > ground,
        "jump should lift the dino: {ground} -> {after_jump}"
    );

    for _ in 0..200 {
        game.tick(0.016, &[], &[]).unwrap();
    }
    let landed = node_y(&game.scene().unwrap(), "dino");
    assert!(
        (landed - ground).abs() < 1e-3,
        "gravity should land the dino back on {ground}, got {landed}"
    );
}

/// Without the action, nothing lifts the dino — the key means nothing to the
/// engine, only to this game's action map.
#[test]
fn dino_does_not_jump_without_the_action() {
    let _s = serial();
    if missing() { return }
    let game = load();
    let ground = node_y(&game.scene().unwrap(), "dino");
    for _ in 0..10 {
        game.tick(0.016, &[], &[]).unwrap();
    }
    assert!((node_y(&game.scene().unwrap(), "dino") - ground).abs() < 1e-3);
}

/// An unrelated key is not the jump action.
#[test]
fn unmapped_key_is_not_the_jump_action() {
    let _s = serial();
    if missing() { return }
    let game = load();
    let ground = node_y(&game.scene().unwrap(), "dino");
    game.tick(0.016, &[keys::code("q").unwrap()], &[]).unwrap();
    assert!((node_y(&game.scene().unwrap(), "dino") - ground).abs() < 1e-3);
}

/// scroller.hom: trees slide left and wrap back to reset_to.
#[test]
fn scroller_moves_left_and_wraps() {
    let _s = serial();
    if missing() { return }
    let game = load();
    let start = node_x(&game.scene().unwrap(), "tree1");

    game.tick(0.1, &[], &[]).unwrap();
    let moved = node_x(&game.scene().unwrap(), "tree1");
    assert!(moved < start, "tree should scroll left: {start} -> {moved}");

    // wrap_at is -7.0, reset_to 7.5; at speed -3.0 that needs ~4s.
    for _ in 0..300 {
        game.tick(0.05, &[], &[]).unwrap();
    }
    let x = node_x(&game.scene().unwrap(), "tree1");
    assert!(x <= 7.5 && x > -7.0, "tree should have wrapped, got {x}");
}

#[test]
fn restart_returns_the_scene_to_its_initial_state() {
    let _s = serial();
    if missing() { return }
    let game = load();
    let start = node_x(&game.scene().unwrap(), "tree1");
    for _ in 0..20 {
        game.tick(0.05, &[], &[]).unwrap();
    }
    assert!(node_x(&game.scene().unwrap(), "tree1") < start);

    game.restart();
    assert!((node_x(&game.scene().unwrap(), "tree1") - start).abs() < 1e-6);
}

// ── game4: a system with no visible effect still runs ───────────────────────

fn game4_so() -> PathBuf {
    examples().join("target/games/libgame4.so")
}

/// Read the live `index` out of the opaque `Dialogue` component. Going through
/// serde rather than the serialized text keeps this independent of how
/// `ron::Value` chooses to render a map.
#[derive(serde::Deserialize)]
struct DialogueState {
    index: i64,
}

fn dialogue_index(scene: &scene::Scene) -> i64 {
    let node = scene
        .nodes
        .iter()
        .find(|n| n.components.contains_key("Dialogue"))
        .expect("game4 should carry a Dialogue component");
    node.components
        .get("Dialogue")
        .unwrap()
        .clone()
        .into_rust::<DialogueState>()
        .expect("Dialogue should carry an index")
        .index
}

/// dialogue.hom advances its index on the "advance" action and stops at the
/// last line. Nothing moves on screen, so this is the only way to see it work.
#[test]
fn game4_dialogue_advances_on_the_action_and_stops_at_the_end() {
    let _s = serial();
    if !game4_so().exists() {
        eprintln!("skipping: {} not built", game4_so().display());
        return;
    }
    let ron_text =
        std::fs::read_to_string(examples().join("assets/games/game4/world.ron")).unwrap();
    let game = GameModule::load(&game4_so(), &ron_text).expect("load libgame4.so");

    assert_eq!(dialogue_index(&game.scene().unwrap()), 0);

    // The action is a one-shot, so each press advances exactly one line.
    game.tick(0.016, &[keys::SPACE], &[keys::SPACE]).unwrap();
    assert_eq!(dialogue_index(&game.scene().unwrap()), 1);

    game.tick(0.016, &[], &[]).unwrap();
    assert_eq!(
        dialogue_index(&game.scene().unwrap()),
        1,
        "no action, no advance"
    );

    for _ in 0..20 {
        game.tick(0.016, &[keys::SPACE], &[keys::SPACE]).unwrap();
    }
    assert_eq!(
        dialogue_index(&game.scene().unwrap()),
        4,
        "should stop on the last of 5 lines"
    );
}

// ── Contacts carry identity, and are recomputed after movement ─────────────

#[derive(serde::Deserialize)]
struct RunState {
    crashes: i64,
    last: String,
}

fn run_state(scene: &scene::Scene) -> RunState {
    scene
        .nodes
        .iter()
        .find(|n| n.components.contains_key("Run"))
        .expect("game3 should carry a Run component")
        .components
        .get("Run")
        .unwrap()
        .clone()
        .into_rust::<RunState>()
        .expect("Run should deserialize")
}

fn node_x_of(scene: &scene::Scene, name: &str) -> f32 {
    scene.nodes.iter().find(|n| n.name == name).unwrap().transform.translation[0]
}

/// The obstacle system names the object it hit, which is only possible because
/// a contact carries node identity rather than component names.
#[test]
fn a_crash_records_which_obstacle_caused_it() {
    let _s = serial();
    if missing() { return }
    let game = load();
    assert_eq!(run_state(&game.scene().unwrap()).crashes, 0);

    // Trees scroll left at 3.0/s from x = 4.0 and 9.5; the dino sits at -3.0.
    // Run long enough for the first to reach it.
    let mut crashed_on = String::new();
    for _ in 0..400 {
        game.tick(0.016, &[], &[]).unwrap();
        let st = run_state(&game.scene().unwrap());
        if st.crashes > 0 {
            crashed_on = st.last;
            break;
        }
    }
    assert!(
        crashed_on == "tree1" || crashed_on == "tree2",
        "should name the obstacle it hit, got {crashed_on:?}"
    );
}

/// Contacts are computed after the movement systems run, so a response acts on
/// this tick's positions. If they were stale the run would restart a tick late
/// and the tree would already have passed through the dino.
#[test]
fn a_crash_restarts_the_run_in_the_same_tick() {
    let _s = serial();
    if missing() { return }
    let game = load();
    let start_x = node_x_of(&game.scene().unwrap(), "tree1");

    for _ in 0..400 {
        game.tick(0.016, &[], &[]).unwrap();
        if run_state(&game.scene().unwrap()).crashes > 0 {
            // restart() put the scene back, so the tree is at its start again.
            let x = node_x_of(&game.scene().unwrap(), "tree1");
            assert!(
                (x - start_x).abs() < 1e-4,
                "restart should reset positions: {start_x} -> {x}"
            );
            return;
        }
    }
    panic!("expected a crash within 400 ticks");
}

/// Jumping over an obstacle avoids the contact entirely — the collider is the
/// dino's, and it leaves the ground.
#[test]
fn jumping_clears_an_obstacle() {
    let _s = serial();
    if missing() { return }
    let game = load();
    // Hold the jump action the whole time: the dino re-jumps every landing.
    for _ in 0..200 {
        game.tick(0.016, &[keys::SPACE], &[keys::SPACE]).unwrap();
    }
    let st = run_state(&game.scene().unwrap());
    assert!(
        st.crashes <= 1,
        "continuous jumping should mostly clear obstacles, got {} crashes",
        st.crashes
    );
}

// ── Hierarchy ──────────────────────────────────────────────────────────────

fn find_child<'a>(scene: &'a scene::Scene, parent: &str, child: &str) -> &'a scene::Node {
    scene
        .nodes
        .iter()
        .find(|n| n.name == parent)
        .unwrap_or_else(|| panic!("no node {parent}"))
        .children
        .iter()
        .find(|n| n.name == child)
        .unwrap_or_else(|| panic!("{parent} has no child {child}"))
}

/// A child node comes back as a child, not flattened into the root list. The
/// runtime is flat; the tree is what the host sees.
#[test]
fn a_child_node_survives_the_round_trip() {
    let _s = serial();
    if missing() { return }
    let game = load();
    let scene = game.scene().unwrap();
    assert!(
        !scene.nodes.iter().any(|n| n.name == "tree1_top"),
        "a child must not appear at the root"
    );
    let child = find_child(&scene, "tree1", "tree1_top");
    assert_eq!(child.transform.translation[1], 0.7, "its local transform");
}

/// Only the parent carries ScrollX, so the child's local transform never
/// changes while the parent's does — the child rides along.
#[test]
fn a_child_rides_its_parent_without_a_system_of_its_own() {
    let _s = serial();
    if missing() { return }
    let game = load();
    let parent_x0 = node_x_of(&game.scene().unwrap(), "tree1");
    let child_y0 = find_child(&game.scene().unwrap(), "tree1", "tree1_top")
        .transform
        .translation[1];

    for _ in 0..20 {
        game.tick(0.016, &[], &[]).unwrap();
    }
    let scene = game.scene().unwrap();
    assert!(
        node_x_of(&scene, "tree1") < parent_x0,
        "the parent scrolled"
    );
    let child = find_child(&scene, "tree1", "tree1_top");
    assert_eq!(
        child.transform.translation[1], child_y0,
        "the child's local transform is untouched"
    );
    assert_eq!(
        child.transform.translation[0], 0.0,
        "and its local x stays at zero: it moves because its parent did"
    );
}

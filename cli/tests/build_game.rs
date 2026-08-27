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
    std::fs::read_to_string(examples().join("assets/games/game3/scene.ron"))
        .expect("read game3 scene")
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
        std::fs::read_to_string(examples().join("assets/games/game4/scene.ron")).unwrap();
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

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use shinra_tui::core::app::{AppState, PanelId};
use shinra_tui::tui::layout;
use std::path::{Path, PathBuf};

fn snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/snapshots")
}

const TERM_W: u16 = 120;
const TERM_H: u16 = 40;

fn buffer_to_text(backend: &TestBackend) -> String {
    let buf = backend.buffer();
    let mut lines = Vec::new();
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf[(x, y)].symbol());
        }
        lines.push(line.trim_end().to_string());
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn check_or_update_txt_snapshot(actual: &str, baseline_path: &Path) {
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        std::fs::create_dir_all(baseline_path.parent().unwrap()).unwrap();
        std::fs::write(baseline_path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(baseline_path).unwrap_or_else(|_| {
        panic!(
            "Baseline not found: {}. Run with UPDATE_SNAPSHOTS=1 to create it.",
            baseline_path.display()
        )
    });
    if actual != expected {
        // Find first differing line for a useful error message
        let actual_lines: Vec<&str> = actual.lines().collect();
        let expected_lines: Vec<&str> = expected.lines().collect();
        let mut diff_line = None;
        for (i, (a, e)) in actual_lines.iter().zip(expected_lines.iter()).enumerate() {
            if a != e {
                diff_line = Some((i + 1, *a, *e));
                break;
            }
        }
        if diff_line.is_none() && actual_lines.len() != expected_lines.len() {
            diff_line = Some((
                actual_lines.len().min(expected_lines.len()) + 1,
                actual_lines
                    .get(expected_lines.len())
                    .or(Some(&"<missing>"))
                    .unwrap(),
                expected_lines
                    .get(actual_lines.len())
                    .or(Some(&"<missing>"))
                    .unwrap(),
            ));
        }
        match diff_line {
            Some((line, actual_text, expected_text)) => {
                panic!(
                    "Snapshot mismatch at {}, line {}:\n  actual:   {:?}\n  expected: {:?}\nRun with UPDATE_SNAPSHOTS=1 to update.",
                    baseline_path.display(),
                    line,
                    actual_text,
                    expected_text
                );
            }
            None => {
                panic!(
                    "Snapshot mismatch at {} (content differs)\nRun with UPDATE_SNAPSHOTS=1 to update.",
                    baseline_path.display()
                );
            }
        }
    }
}

fn render_snapshot(app: &AppState) -> String {
    let backend = TestBackend::new(TERM_W, TERM_H);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            layout::draw(f, app, None);
        })
        .unwrap();
    buffer_to_text(terminal.backend())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

const GAME_RON: &str = r#"(
    name: "test",
    input: "input.tres.ron",
    views: {
        // World-rooted: these tests edit a world, and there is no UI over it.
        "main": ( unit: World("world.ron"), graphics: View3D,
                  camera: ( projection: Perspective( fov_y_degrees: 60.0, znear: 0.1, zfar: 100.0 ) ) ),
    },
)"#;

fn write_test_scene(dir: &std::path::Path) {
    let scene = scene::Scene {
        name: "test".into(),
        nodes: vec![scene::Node {
            name: "player".into(),
            transform: scene::Transform {
                translation: [0.0, 0.0, 0.0],
                ..Default::default()
            },
            mesh: Some(scene::MeshRef {
                path: "assets/bunny.obj".into(),
            }),
            ..Default::default()
        }],
    };
    // A game folder is one with a game.ron; world.ron is what the IDE draws.
    std::fs::write(dir.join("game.ron"), GAME_RON).unwrap();
    scene.save(&dir.join("world.ron")).unwrap();
}

fn make_app() -> (AppState, std::path::PathBuf, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp.path().join("game1");
    std::fs::create_dir(&game_dir).unwrap();
    write_test_scene(&game_dir);
    let app = AppState::new_headless(tmp.path()).unwrap();
    (app, game_dir, tmp)
}

// -- Snapshot tests --

#[test]
fn snapshot_initial_layout() {
    let (app, _, _tmp) = make_app();
    let snapshot = render_snapshot(&app);

    let baseline = snapshot_dir().join("bunny/initial.tui.txt");
    check_or_update_txt_snapshot(&snapshot, &baseline);
}

#[test]
fn snapshot_after_select_node() {
    let (mut app, _, _tmp) = make_app();

    app.focused = PanelId::Hierarchy;
    app.handle_key(key(KeyCode::Down));

    let snapshot = render_snapshot(&app);

    let baseline = snapshot_dir().join("bunny/after-select-node.tui.txt");
    check_or_update_txt_snapshot(&snapshot, &baseline);
}

#[test]
fn snapshot_after_adjust_x() {
    let (mut app, _, _tmp) = make_app();

    app.focused = PanelId::Hierarchy;
    app.handle_key(key(KeyCode::Down));
    app.focused = PanelId::Inspector;
    app.handle_key(key(KeyCode::Char('e')));
    app.handle_key(key(KeyCode::Char('+')));

    let snapshot = render_snapshot(&app);

    let baseline = snapshot_dir().join("bunny/after-move-x.tui.txt");
    check_or_update_txt_snapshot(&snapshot, &baseline);
}

/// Play mode is scheduling only. Anything drawn over the game picture is a
/// canvas node the game declared, so entering play mode adds nothing.
#[test]
fn play_mode_draws_nothing_of_its_own() {
    let (mut app, _, _tmp) = make_app();

    let before = render_snapshot(&app);
    app.toggle_run();
    assert!(app.run.is_some());
    let during = render_snapshot(&app);

    // The status line changes; nothing else the IDE owns appears.
    for marker in ["Space: next", "End of dialogue", "run-messages"] {
        assert!(!during.contains(marker), "the IDE drew {marker}");
        assert!(!before.contains(marker));
    }

    app.handle_key(key(KeyCode::Esc));
    assert!(app.run.is_none());
}

/// The IDE's status line must not describe gameplay keys — it does not know
/// what a game binds them to.
#[test]
fn play_mode_status_names_only_ide_keys() {
    let (mut app, _, _tmp) = make_app();
    app.toggle_run();
    let frame = render_snapshot(&app);
    assert!(frame.contains("RUNNING"));
    for gameplay in ["space jump", "space next text", "Space: next"] {
        assert!(
            !frame.contains(gameplay),
            "status line leaked a gameplay key: {gameplay}"
        );
    }
}

/// The engine stores game components opaquely: game4 carries a `Dialogue`
/// component the scene crate has never heard of, and it must still load. The
/// dialogue text itself lives in a resource, not in the scene.
#[test]
fn game4_scene_carries_an_opaque_component_and_no_dialogue_text() {
    let games = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("shinra-examples/assets/games/game4");

    let scene = scene::Scene::load(&games.join("world.ron")).unwrap();
    assert_eq!(scene.name, "terminal-hearts");

    let story = scene
        .nodes
        .iter()
        .find(|n| n.components.contains_key("Dialogue"))
        .expect("game4 should carry an opaque Dialogue component");
    assert!(
        story.mesh.is_none() && story.sprite.is_none(),
        "the story node draws nothing"
    );

    let raw = std::fs::read_to_string(games.join("world.ron")).unwrap();
    assert!(
        !raw.contains("speaker:"),
        "dialogue text must not live in world.ron"
    );
    assert!(games.join("story.tres.ron").exists(), "story resource missing");
}

// -- Persistence test --

#[test]
fn adjust_and_save_persists_to_disk() {
    let (mut app, game_dir, _tmp) = make_app();

    app.focused = PanelId::Hierarchy;
    app.handle_key(key(KeyCode::Down));
    app.focused = PanelId::Inspector;
    app.handle_key(key(KeyCode::Char('e')));
    app.handle_key(key(KeyCode::Char('+')));
    app.save_scene();

    let loaded = scene::Scene::load(&game_dir.join("world.ron")).unwrap();
    let x = loaded.nodes[0].transform.translation[0];
    assert!(
        (x - 0.1).abs() < 1e-6,
        "expected translation X ~0.1, got {x}"
    );
}

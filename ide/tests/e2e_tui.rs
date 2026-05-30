use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use shinra_ide::core::app::{AppState, PanelId};
use shinra_ide::tui::layout;

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

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn write_test_scene(dir: &std::path::Path) {
    let scene = scene::Scene {
        name: "test".into(),
        camera: None,
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
    scene.save(&dir.join("scene.ron")).unwrap();
}

#[test]
fn adjust_and_save_persists_to_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp.path().join("game1");
    std::fs::create_dir(&game_dir).unwrap();
    write_test_scene(&game_dir);

    let mut app = AppState::new_headless(tmp.path()).unwrap();

    // Focus hierarchy, select the node
    app.focused = PanelId::Hierarchy;
    app.handle_key(key(KeyCode::Down));

    // Focus inspector, enter edit mode, adjust X by +0.1
    app.focused = PanelId::Inspector;
    app.handle_key(key(KeyCode::Char('e')));
    app.handle_key(key(KeyCode::Char('+')));

    // Save
    app.save_scene();

    // Read back and verify
    let scene_path = game_dir.join("scene.ron");
    let loaded = scene::Scene::load(&scene_path).unwrap();
    let x = loaded.nodes[0].transform.translation[0];
    assert!(
        (x - 0.1).abs() < 1e-6,
        "expected translation X ~0.1, got {x}"
    );
}

#[test]
fn full_layout_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp.path().join("game1");
    std::fs::create_dir(&game_dir).unwrap();
    write_test_scene(&game_dir);

    let app = AppState::new_headless(tmp.path()).unwrap();

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            layout::draw(f, &app, None);
        })
        .unwrap();

    let snapshot = buffer_to_text(terminal.backend());

    // All 6 panel areas should be present in the layout
    assert!(snapshot.contains("Hierarchy"), "missing Hierarchy panel");
    assert!(snapshot.contains("Viewport"), "missing Viewport panel");
    assert!(snapshot.contains("Inspector"), "missing Inspector panel");
    assert!(snapshot.contains("Project"), "missing Project panel");
    assert!(snapshot.contains("Terminal"), "missing Terminal panel");
    assert!(snapshot.contains("File"), "missing menu bar");

    // Verify the snapshot has reasonable dimensions
    let line_count = snapshot.lines().count();
    assert!(
        line_count >= 30,
        "snapshot too short: {line_count} lines (expected ~40)"
    );
}

#[test]
fn snapshot_shows_inspector_values_after_edit() {
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp.path().join("game1");
    std::fs::create_dir(&game_dir).unwrap();
    write_test_scene(&game_dir);

    let mut app = AppState::new_headless(tmp.path()).unwrap();

    // Select node and adjust X
    app.focused = PanelId::Hierarchy;
    app.handle_key(key(KeyCode::Down));
    app.focused = PanelId::Inspector;
    app.handle_key(key(KeyCode::Char('e')));
    app.handle_key(key(KeyCode::Char('+')));

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            layout::draw(f, &app, None);
        })
        .unwrap();

    let snapshot = buffer_to_text(terminal.backend());

    // Inspector should show the updated X value (0.100)
    assert!(
        snapshot.contains("0.100"),
        "inspector should show adjusted X value 0.100 in snapshot:\n{snapshot}"
    );

    // Node name should appear
    assert!(
        snapshot.contains("player"),
        "inspector should show node name 'player'"
    );
}

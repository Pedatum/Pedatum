use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use shinra_ide::core::app::{AppState, PanelId};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn base_scene() -> scene::Scene {
    scene::Scene {
        name: "parity_test".into(),
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
    }
}

#[test]
fn tui_and_gui_produce_identical_ron() {
    let dir_tui = tempfile::tempdir().unwrap();
    let dir_gui = tempfile::tempdir().unwrap();

    // --- TUI path ---
    let game_dir = dir_tui.path().join("game1");
    std::fs::create_dir(&game_dir).unwrap();
    base_scene().save(&game_dir.join("scene.ron")).unwrap();

    let mut app = AppState::new_headless(dir_tui.path()).unwrap();

    app.focused = PanelId::Hierarchy;
    app.handle_key(key(KeyCode::Down));

    app.focused = PanelId::Inspector;
    app.handle_key(key(KeyCode::Char('e')));
    app.handle_key(key(KeyCode::Char('+')));

    app.save_scene();
    let tui_ron = std::fs::read_to_string(game_dir.join("scene.ron")).unwrap();

    // --- GUI path ---
    let mut gui_scene = base_scene();
    gui_scene.nodes[0].transform.translation[0] += 0.1;
    let gui_path = dir_gui.path().join("scene.ron");
    gui_scene.save(&gui_path).unwrap();
    let gui_ron = std::fs::read_to_string(&gui_path).unwrap();

    // --- Assert parity ---
    assert_eq!(
        tui_ron, gui_ron,
        "TUI and GUI should produce identical scene.ron"
    );
}

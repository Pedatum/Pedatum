use scene;

fn test_scene() -> scene::Scene {
    scene::Scene {
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
    }
}

#[test]
fn save_load_roundtrip_with_mutation() {
    let mut scene = test_scene();

    scene.nodes[0].transform.translation[0] += 1.0;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.ron");
    scene.save(&path).unwrap();

    let loaded = scene::Scene::load(&path).unwrap();

    assert_eq!(loaded.nodes[0].transform.translation[0], 1.0);
    assert_eq!(loaded.nodes[0].name, "player");
}

#[test]
#[ignore] // requires GPU — run with: cargo test -p editor -- --ignored
fn engine_renders_mutated_scene() {
    let mut scene = test_scene();
    scene.nodes[0].transform.translation[0] += 1.0;

    let mut engine = shinra_engine::Engine::new(256, 192);

    use shinra_engine::EngineBackend;
    let backend: &mut dyn EngineBackend = &mut engine;
    backend.load_scene(&scene);
    backend.render();

    let img = backend.frame_image();
    let snapshot_path = std::path::Path::new("tests/fixtures/mutated_scene.png");
    img.save(snapshot_path).unwrap();

    let has_color = img.pixels().any(|p| p.0[0] > 10 || p.0[1] > 10 || p.0[2] > 10);
    assert!(has_color, "rendered image should not be all black");
}

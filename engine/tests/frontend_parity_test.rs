use shinra_engine::{engine::Engine, EngineBackend};
use std::path::Path;

#[test]
fn frontend_parity_single_codepath() {
    let scene_data = std::fs::read_to_string("tests/fixtures/cube.ron").unwrap();
    let scene: scene_format::Scene = ron::from_str(&scene_data).unwrap();

    let mut engine = Engine::new(256, 144);

    // Both frontends ultimately call the same EngineBackend methods:
    //   backend.load_scene(scene) → backend.render() → backend.frame_image()
    // Exercise via the trait to prove there is ONE code path.
    let backend: &mut dyn EngineBackend = &mut engine;
    backend.load_scene(&scene);
    backend.render();
    let img_a = backend.frame_image();

    // Render again through the same trait — deterministic output.
    backend.render();
    let img_b = backend.frame_image();

    assert_eq!(img_a.dimensions(), (256, 144));
    assert_eq!(img_b.dimensions(), (256, 144));
    assert_eq!(
        img_a.as_raw(),
        img_b.as_raw(),
        "same scene rendered twice through EngineBackend must produce identical frames"
    );

    // Verify actual content was rendered (not just clear color).
    let has_content = img_a.pixels().any(|p| p.0[0] > 80);
    assert!(has_content, "frame should contain rasterized geometry");
}

#[test]
fn frontend_parity_snapshot_roundtrip() {
    let scene_data = std::fs::read_to_string("tests/fixtures/cube.ron").unwrap();
    let scene: scene_format::Scene = ron::from_str(&scene_data).unwrap();

    let mut engine = Engine::new(256, 144);
    let backend: &mut dyn EngineBackend = &mut engine;
    backend.load_scene(&scene);
    backend.render();

    let dir = Path::new("target/debug/parity");
    std::fs::create_dir_all(dir).unwrap();
    let snap_path = dir.join("parity_cube.png");
    backend.snapshot(&snap_path).unwrap();

    // Re-read the saved snapshot and compare to frame_image output.
    let saved = image::open(&snap_path).unwrap().to_rgba8();
    let live = backend.frame_image();

    assert_eq!(saved.dimensions(), live.dimensions());
    assert_eq!(
        saved.as_raw(),
        live.as_raw(),
        "snapshot on disk must match frame_image output exactly"
    );
}

#[test]
fn frontend_parity_trait_object_and_concrete_match() {
    let scene_data = std::fs::read_to_string("tests/fixtures/cube.ron").unwrap();
    let scene: scene_format::Scene = ron::from_str(&scene_data).unwrap();

    // Render via concrete Engine methods.
    let mut engine = Engine::new(256, 144);
    engine.load_scene(&scene);
    engine.render_current();
    let img_concrete = engine.frame_image();

    // Render via EngineBackend trait (what both frontends use).
    let mut engine2 = Engine::new(256, 144);
    let backend: &mut dyn EngineBackend = &mut engine2;
    backend.load_scene(&scene);
    backend.render();
    let img_trait = backend.frame_image();

    assert_eq!(
        img_concrete.as_raw(),
        img_trait.as_raw(),
        "concrete Engine calls and EngineBackend trait calls must produce identical output"
    );
}

use shinra_engine::engine::Engine;
use std::path::Path;

fn render_scene_to_png(scene_path: &str, output: &str, width: u32, height: u32) {
    let scene_data = std::fs::read_to_string(scene_path).unwrap();
    let scene: scene_format::Scene = ron::from_str(&scene_data).unwrap();

    let mut engine = Engine::new(width, height);
    engine.load_scene(&scene);
    engine.render_current();

    let out_dir = Path::new("target/debug/snapshots");
    std::fs::create_dir_all(out_dir).unwrap();
    engine.snapshot(&out_dir.join(output)).unwrap();
}

#[test]
fn snapshot_cube_scene() {
    render_scene_to_png("tests/fixtures/cube.ron", "cube_snapshot.png", 256, 144);
    let path = Path::new("target/debug/snapshots/cube_snapshot.png");
    assert!(path.exists());
    let img = image::open(path).unwrap();
    assert_eq!(img.width(), 256);
    assert_eq!(img.height(), 144);
}

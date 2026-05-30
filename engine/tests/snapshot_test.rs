use shinra_engine::engine::Engine;
use std::path::{Path, PathBuf};

fn snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/snapshots")
}

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

fn check_or_update_snapshot(actual: &Path, baseline: &Path, tolerance: u8) {
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        std::fs::create_dir_all(baseline.parent().unwrap()).unwrap();
        std::fs::copy(actual, baseline).unwrap();
        return;
    }
    assert!(
        compare_snapshots(actual, baseline, tolerance),
        "Snapshot mismatch: {} vs {}",
        actual.display(),
        baseline.display()
    );
}

fn compare_snapshots(actual: &Path, expected: &Path, tolerance: u8) -> bool {
    let actual_img = image::open(actual).unwrap().to_rgba8();
    let expected_img = image::open(expected).unwrap().to_rgba8();

    assert_eq!(
        actual_img.dimensions(),
        expected_img.dimensions(),
        "size mismatch"
    );

    let (w, h) = actual_img.dimensions();
    let mut max_diff: u8 = 0;
    for y in 0..h {
        for x in 0..w {
            let a = actual_img.get_pixel(x, y);
            let e = expected_img.get_pixel(x, y);
            for c in 0..4 {
                let diff = (a[c] as i16 - e[c] as i16).unsigned_abs() as u8;
                max_diff = max_diff.max(diff);
            }
        }
    }
    max_diff <= tolerance
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

#[test]
fn snapshot_compare_identical() {
    render_scene_to_png("tests/fixtures/cube.ron", "cube_cmp_a.png", 256, 144);
    render_scene_to_png("tests/fixtures/cube.ron", "cube_cmp_b.png", 256, 144);

    let dir = Path::new("target/debug/snapshots");
    assert!(compare_snapshots(
        &dir.join("cube_cmp_a.png"),
        &dir.join("cube_cmp_b.png"),
        2
    ));
}

#[test]
fn snapshot_cube_matches_baseline() {
    render_scene_to_png("tests/fixtures/cube.ron", "cube_baseline_check.png", 256, 144);

    let actual = Path::new("target/debug/snapshots/cube_baseline_check.png");
    let baseline = snapshot_dir().join("cube/initial.engine.png");

    check_or_update_snapshot(actual, &baseline, 2);
}

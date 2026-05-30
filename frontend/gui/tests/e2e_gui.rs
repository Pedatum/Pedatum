use scene;
use std::path::{Path, PathBuf};

fn snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/snapshots")
}

const RENDER_W: u32 = 256;
const RENDER_H: u32 = 192;

fn test_scene() -> scene::Scene {
    scene::Scene {
        name: "test".into(),
        camera: Some(scene::Camera {
            eye: [2.0, 2.0, 2.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            projection: scene::Projection::Perspective {
                fov_y_degrees: 60.0,
                aspect: RENDER_W as f32 / RENDER_H as f32,
                znear: 0.1,
                zfar: 100.0,
            },
        }),
        nodes: vec![scene::Node {
            name: "cube".into(),
            transform: scene::Transform {
                translation: [0.0, 0.0, 0.0],
                ..Default::default()
            },
            mesh: Some(scene::MeshRef {
                path: "../../engine/tests/fixtures/cube.obj".into(),
            }),
            ..Default::default()
        }],
    }
}

fn render_scene_to_image(scene: &scene::Scene) -> image::RgbaImage {
    let mut engine = shinra_engine::Engine::new(RENDER_W, RENDER_H);
    use shinra_engine::EngineBackend;
    let backend: &mut dyn EngineBackend = &mut engine;
    backend.load_scene(scene);
    backend.render();
    backend.frame_image()
}

fn compare_png(actual: &Path, baseline: &Path, tolerance: u8) -> bool {
    let actual_img = image::open(actual).unwrap().to_rgba8();
    let expected_img = image::open(baseline).unwrap().to_rgba8();
    if actual_img.dimensions() != expected_img.dimensions() {
        return false;
    }
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

fn check_or_update_png_snapshot(img: &image::RgbaImage, baseline_path: &Path) {
    let actual_dir = Path::new("target/debug/e2e_snapshots");
    std::fs::create_dir_all(actual_dir).unwrap();
    let actual_path = actual_dir.join(baseline_path.file_name().unwrap());
    img.save(&actual_path).unwrap();

    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        std::fs::create_dir_all(baseline_path.parent().unwrap()).unwrap();
        std::fs::copy(&actual_path, baseline_path).unwrap();
        return;
    }

    if !baseline_path.exists() {
        panic!(
            "Baseline not found: {}. Run with UPDATE_SNAPSHOTS=1 to create it.",
            baseline_path.display()
        );
    }

    assert!(
        compare_png(&actual_path, baseline_path, 2),
        "PNG snapshot mismatch: {} vs {}. Run with UPDATE_SNAPSHOTS=1 to update.",
        actual_path.display(),
        baseline_path.display()
    );
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
    assert_eq!(loaded.nodes[0].name, "cube");
}

#[test]
fn snapshot_before_mutation() {
    let scene = test_scene();
    let img = render_scene_to_image(&scene);

    let baseline = snapshot_dir().join("bunny/initial.gui.png");
    check_or_update_png_snapshot(&img, &baseline);
}

#[test]
fn snapshot_after_mutation() {
    let mut scene = test_scene();
    scene.nodes[0].transform.translation[0] += 1.0;
    let img = render_scene_to_image(&scene);

    let baseline = snapshot_dir().join("bunny/after-move-x.gui.png");
    check_or_update_png_snapshot(&img, &baseline);
}

#[test]
fn before_and_after_differ() {
    if !Path::new("../../engine/tests/fixtures/cube.obj").exists() {
        eprintln!("skipping before_and_after_differ: cube.obj not found");
        return;
    }

    let before_img = render_scene_to_image(&test_scene());

    let mut scene = test_scene();
    scene.nodes[0].transform.translation[0] += 1.0;
    let after_img = render_scene_to_image(&scene);

    let dir = tempfile::tempdir().unwrap();
    let before_path = dir.path().join("before.png");
    let after_path = dir.path().join("after.png");
    before_img.save(&before_path).unwrap();
    after_img.save(&after_path).unwrap();

    assert!(
        !compare_png(&before_path, &after_path, 2),
        "before and after mutation should produce different renders"
    );
}

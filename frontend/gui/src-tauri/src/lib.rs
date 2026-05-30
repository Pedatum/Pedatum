use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct SceneData {
    pub name: String,
    pub nodes: Vec<NodeData>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeData {
    pub name: String,
    pub transform: TransformData,
    pub mesh: Option<MeshData>,
    pub children: Vec<NodeData>,
}

#[derive(Serialize, Deserialize)]
pub struct TransformData {
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Serialize, Deserialize)]
pub struct MeshData {
    pub path: String,
}

#[tauri::command]
fn load_scene(path: String) -> Result<SceneData, String> {
    let scene = scene_format::Scene::load(&PathBuf::from(&path))
        .map_err(|e| format!("Failed to load scene: {e}"))?;

    Ok(scene_to_data(&scene))
}

#[tauri::command]
fn save_scene(scene: SceneData, path: String) -> Result<(), String> {
    let s = data_to_scene(&scene);
    s.save(&PathBuf::from(&path))
        .map_err(|e| format!("Failed to save scene: {e}"))?;
    Ok(())
}

fn scene_to_data(scene: &scene_format::Scene) -> SceneData {
    SceneData {
        name: scene.name.clone(),
        nodes: scene.nodes.iter().map(node_to_data).collect(),
    }
}

fn node_to_data(node: &scene_format::Node) -> NodeData {
    NodeData {
        name: node.name.clone(),
        transform: TransformData {
            translation: node.transform.translation,
            rotation: node.transform.rotation,
            scale: node.transform.scale,
        },
        mesh: node.mesh.as_ref().map(|m| MeshData {
            path: m.path.clone(),
        }),
        children: node.children.iter().map(node_to_data).collect(),
    }
}

fn data_to_scene(data: &SceneData) -> scene_format::Scene {
    scene_format::Scene {
        name: data.name.clone(),
        camera: None,
        nodes: data.nodes.iter().map(data_to_node).collect(),
    }
}

fn data_to_node(data: &NodeData) -> scene_format::Node {
    scene_format::Node {
        name: data.name.clone(),
        transform: scene_format::Transform {
            translation: data.transform.translation,
            rotation: data.transform.rotation,
            scale: data.transform.scale,
        },
        mesh: data.mesh.as_ref().map(|m| scene_format::MeshRef {
            path: m.path.clone(),
        }),
        children: data.children.iter().map(data_to_node).collect(),
        ..Default::default()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![load_scene, save_scene])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}

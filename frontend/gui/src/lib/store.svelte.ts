export interface Transform {
  translation: [number, number, number];
  rotation: [number, number, number];
  scale: [number, number, number];
}

export interface SceneNode {
  name: string;
  transform: Transform;
  mesh?: { path: string };
  children: SceneNode[];
}

export interface Camera {
  eye: [number, number, number];
  target: [number, number, number];
  up: [number, number, number];
}

export interface Scene {
  name: string;
  camera: Camera | null;
  nodes: SceneNode[];
}

function defaultTransform(): Transform {
  return { translation: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] };
}

function defaultScene(): Scene {
  return {
    name: "untitled",
    camera: {
      eye: [2, 2, 2],
      target: [0, 0, 0],
      up: [0, 1, 0],
    },
    nodes: [
      {
        name: "cube",
        transform: defaultTransform(),
        mesh: { path: "engine/tests/fixtures/cube.obj" },
        children: [],
      },
    ],
  };
}

class AppState {
  scene = $state<Scene>(defaultScene());
  selectedNodeIndex = $state<number>(0);

  get selectedNode(): SceneNode | undefined {
    return this.scene.nodes[this.selectedNodeIndex];
  }

  selectNode(index: number) {
    this.selectedNodeIndex = index;
  }

  async loadScene(path: string): Promise<void> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      this.scene = await invoke<Scene>("load_scene", { path });
    } catch {
      console.log("Tauri not available, using default scene");
    }
  }

  async saveScene(path: string): Promise<void> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_scene", { scene: this.scene, path });
    } catch {
      console.log("Tauri not available, save skipped");
    }
  }
}

export const app = new AppState();

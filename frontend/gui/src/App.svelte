<script lang="ts">
  import "./app.css";
  import Hierarchy from "./lib/Hierarchy.svelte";
  import Project from "./lib/Project.svelte";
  import Viewport from "./lib/Viewport.svelte";
  import Inspector from "./lib/Inspector.svelte";
  import Terminal from "./lib/Terminal.svelte";
  import Console from "./lib/Console.svelte";

  import { scene, selectedNodeIndex, loadScene, saveScene } from "./lib/store.svelte.ts";
</script>

<div class="ide-layout" data-testid="ide-layout">
  <div class="left-panels">
    <Hierarchy />
    <Project />
  </div>

  <Viewport />

  <Inspector />

  <div class="bottom-panels">
    <Terminal />
    <Console />
  </div>
</div>

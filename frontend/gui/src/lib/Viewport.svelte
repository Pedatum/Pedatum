<script lang="ts">
  import { app } from "./store.svelte.ts";
</script>

<div class="panel viewport" data-testid="viewport-panel">
  <div style="text-align: center; color: var(--text-dim);">
    <div style="font-size: 48px; margin-bottom: 8px;">🎮</div>
    <div>Viewport — {app.scene.name}</div>
    <div style="font-size: 11px; margin-top: 4px;">
      {app.scene.nodes.length} node(s) in scene
    </div>
    <canvas
      id="viewport-canvas"
      data-testid="viewport-canvas"
      width="640"
      height="480"
      style="display: none;"
    ></canvas>
  </div>
</div>

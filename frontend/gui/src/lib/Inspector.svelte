<script lang="ts">
  import { app } from "./store.svelte.ts";

  let node = $derived(app.selectedNode);
</script>

<div class="panel" data-testid="inspector-panel">
  <div class="panel-header">Inspector</div>
  <div class="panel-content">
    {#if node}
      <div class="field-row">
        <span class="field-label">Name</span>
        <span class="field-value" data-testid="inspector-name">{node.name}</span>
      </div>

      <div style="margin-top: 8px; color: var(--text-dim); font-size: 11px">TRANSFORM</div>

      <div class="field-row">
        <span class="field-label">Position</span>
        <span class="field-value" data-testid="inspector-position">
          {node.transform.translation.map((v) => v.toFixed(2)).join(", ")}
        </span>
      </div>
      <div class="field-row">
        <span class="field-label">Rotation</span>
        <span class="field-value" data-testid="inspector-rotation">
          {node.transform.rotation.map((v) => v.toFixed(2)).join(", ")}
        </span>
      </div>
      <div class="field-row">
        <span class="field-label">Scale</span>
        <span class="field-value" data-testid="inspector-scale">
          {node.transform.scale.map((v) => v.toFixed(2)).join(", ")}
        </span>
      </div>

      {#if node.mesh}
        <div style="margin-top: 8px; color: var(--text-dim); font-size: 11px">MESH</div>
        <div class="field-row">
          <span class="field-label">Path</span>
          <span class="field-value" data-testid="inspector-mesh">{node.mesh.path}</span>
        </div>
      {/if}
    {:else}
      <div style="color: var(--text-dim)">No node selected</div>
    {/if}
  </div>
</div>

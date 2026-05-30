<script lang="ts">
  import { app } from "./store.svelte.ts";
</script>

<div class="panel" data-testid="hierarchy-panel">
  <div class="panel-header">Hierarchy</div>
  <div class="panel-content">
    {#each app.scene.nodes as node, i}
      <div
        class="tree-item"
        class:selected={app.selectedNodeIndex === i}
        data-testid="hierarchy-node-{i}"
        onclick={() => app.selectNode(i)}
        onkeydown={(e) => e.key === "Enter" && app.selectNode(i)}
        role="button"
        tabindex="0"
      >
        {node.mesh ? "🧊" : "📁"} {node.name}
      </div>
      {#each node.children as child, j}
        <div
          class="tree-item"
          style="padding-left: 16px"
          data-testid="hierarchy-child-{i}-{j}"
        >
          {child.mesh ? "🧊" : "📁"} {child.name}
        </div>
      {/each}
    {/each}
  </div>
</div>

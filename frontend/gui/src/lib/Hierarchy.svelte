<script lang="ts">
  import { scene, selectedNodeIndex } from "./store.svelte.ts";

  function selectNode(index: number) {
    selectedNodeIndex = index;
  }
</script>

<div class="panel" data-testid="hierarchy-panel">
  <div class="panel-header">Hierarchy</div>
  <div class="panel-content">
    {#each scene.nodes as node, i}
      <div
        class="tree-item"
        class:selected={selectedNodeIndex === i}
        data-testid="hierarchy-node-{i}"
        onclick={() => selectNode(i)}
        onkeydown={(e) => e.key === "Enter" && selectNode(i)}
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

<script lang="ts">
  let files = $state([
    { name: "scene.ron", type: "file" as const, depth: 0 },
    { name: "assets/", type: "dir" as const, depth: 0 },
    { name: "cube.obj", type: "file" as const, depth: 1 },
  ]);
</script>

<div class="panel" data-testid="project-panel">
  <div class="panel-header">Project</div>
  <div class="panel-content">
    {#each files as file}
      <div
        class="tree-item"
        style="padding-left: {file.depth * 16}px"
        data-testid="project-item-{file.name}"
      >
        {file.type === "dir" ? "📂" : "📄"} {file.name}
      </div>
    {/each}
  </div>
</div>

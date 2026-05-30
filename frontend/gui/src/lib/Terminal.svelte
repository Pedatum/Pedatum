<script lang="ts">
  let lines = $state([
    "$ shinra-engine --scene scene.ron",
    "[info] Engine initialized (wgpu backend)",
    "[info] Scene loaded: 1 node(s)",
    "[info] Listening on :1420",
  ]);
</script>

<div class="panel" data-testid="terminal-panel">
  <div class="panel-header">Terminal</div>
  <div class="panel-content" style="font-size: 12px; line-height: 1.5;">
    {#each lines as line}
      <div style="color: var(--text-dim);">{line}</div>
    {/each}
    <div>$ <span style="opacity: 0.5;">_</span></div>
  </div>
</div>

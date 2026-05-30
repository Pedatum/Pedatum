<script lang="ts">
  let messages = $state([
    { level: "info", text: "Scene loaded successfully" },
    { level: "info", text: "Render pipeline ready" },
  ]);
</script>

<div class="panel" data-testid="console-panel">
  <div class="panel-header">Console</div>
  <div class="panel-content" style="font-size: 12px; line-height: 1.5;">
    {#each messages as msg}
      <div style="color: {msg.level === 'error' ? '#f38ba8' : msg.level === 'warn' ? '#fab387' : 'var(--text-dim)'};">
        [{msg.level}] {msg.text}
      </div>
    {/each}
  </div>
</div>

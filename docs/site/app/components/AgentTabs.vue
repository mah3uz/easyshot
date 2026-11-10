<script setup lang="ts">
const ways = {
  MCP: {
    note: 'A Model Context Protocol server, for agents that prefer tools to shell commands. Register it with your client:',
    code: `# Claude Code\n${mcpClients['Claude Code'].code}\n# Gemini CLI\n${mcpClients['Gemini CLI'].code}`,
  },
  Skill: {
    note: 'A short guide that teaches your agent to use <code>easyshot agent</code>. One command installs it for Claude Code, Codex, Gemini CLI and Cursor. Start a new agent session afterwards.',
    code: 'easyshot agent skill install\n# or into the current folder, to commit with a project\neasyshot agent skill install --project',
  },
  CLI: {
    note: 'Any agent or script can run the commands itself. <code>agent list windows</code> gives each window\'s id, and with <code>--json</code> every command prints exactly one JSON object, errors included.',
    code: 'easyshot agent capture --window 0x55d1c7a4e2b0 --json',
  },
}

type Way = keyof typeof ways
const names = Object.keys(ways) as Way[]
const active = ref<Way>('MCP')

const captureReply =
  '{"ok": true, "kind": "capture", "path": "/home/you/.local/share/easyshot/library/easyshot_2026-11-11-10-24-07.png", "width": 1920, "height": 1200, "scale": 1.5, "window": {"id": "0x55d1c7a4e2b0", "app_id": "firefox", "title": "Pull requests", "x": 0, "y": 0, "width": 1280, "height": 800, "output": "DP-1", "focused": true}, "redaction": {"applied": true, "regions": 2}}'
</script>

<template>
  <div class="grid gap-4">
    <TabPills v-model="active" :names="names" label="Ways in" class="sm:w-fit" />
    <SwapPanel :swap-key="active">
      <div class="agent-panel grid gap-3" role="tabpanel">
        <p class="text-[14.5px] text-fg-2" v-html="ways[active].note" />
        <CodeBlock :code="ways[active].code" prompt />
        <template v-if="active === 'MCP'">
          <CodeBlock label="Codex · ~/.codex/config.toml" :code="mcpClients.Codex.code" />
          <ul class="flex flex-wrap gap-1.5 pt-1" aria-label="MCP tools">
            <li v-for="t in mcpTools" :key="t"><code>{{ t }}</code></li>
          </ul>
        </template>
        <CodeBlock v-else-if="active === 'CLI'" label="Prints" :code="captureReply" class="[&_pre]:whitespace-pre-wrap [&_pre]:[overflow-wrap:anywhere]" />
      </div>
    </SwapPanel>
  </div>
</template>

<style scoped>
.agent-panel :deep(p code),
.agent-panel li code {
  border-radius: 6px;
  background: var(--surface-2);
  border: 1px solid var(--line);
  padding: 1px 6px;
  font-size: 13px;
  color: var(--fg);
}
</style>

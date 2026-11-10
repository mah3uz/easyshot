<script setup lang="ts">
const names = Object.keys(mcpClients) as McpClient[]
const active = ref<McpClient>('Claude Code')
</script>

<template>
  <div class="grid gap-4">
    <TabPills v-model="active" :names="names" label="Agent client" class="sm:w-fit" />
    <SwapPanel :swap-key="active">
      <div class="grid gap-3" role="tabpanel">
        <CodeBlock :code="mcpClients[active].code" :prompt="mcpClients[active].prompt" class="[&_pre]:whitespace-pre-wrap" />
        <p class="text-[14.5px]" v-html="mcpClients[active].note" />
      </div>
    </SwapPanel>
  </div>
</template>

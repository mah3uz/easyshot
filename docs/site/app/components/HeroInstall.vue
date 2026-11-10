<script setup lang="ts">
// The one-line install command under the hero buttons.
const command = 'curl -fsSL https://easyshot.site/install.sh | sh'
const { copied, copy } = useCopy()
</script>

<template>
  <div class="grid w-full max-w-[560px] grid-cols-[minmax(0,1fr)] justify-items-center gap-3">
    <div class="flex h-11 w-fit max-w-full items-center rounded-full border border-line-strong bg-surface pl-4 font-mono text-[13.5px] shadow-soft backdrop-blur-xl">
      <span class="shrink-0 pr-2.5 text-fg-3 select-none" aria-hidden="true">$</span>
      <code class="min-w-0 overflow-x-auto whitespace-nowrap text-fg [scrollbar-width:none] max-sm:[mask-image:linear-gradient(90deg,#000_calc(100%-20px),transparent)] max-sm:pr-5">{{ command }}</code>
      <button
        type="button"
        class="ml-3 grid h-full shrink-0 cursor-pointer grid-flow-col items-center gap-1.5 rounded-r-full border-l border-line pr-4 pl-3 font-sans text-[13px] font-semibold text-fg-2 hover-motion hover:bg-accent-soft hover:text-accent"
        :class="copied && 'text-accent'"
        :aria-label="copied ? 'Copied' : 'Copy the install command'"
        @click="copy(command)"
      >
        <CopyIcon :copied="copied" :size="14" />
        <span class="grid text-left" aria-hidden="true">
          <span class="col-start-1 row-start-1 transition-opacity duration-300" :class="copied && 'opacity-0'">Copy</span>
          <span class="col-start-1 row-start-1 transition-opacity duration-300" :class="!copied && 'opacity-0'">Copied</span>
        </span>
      </button>
    </div>
    <p class="text-[13px] text-balance text-fg-3">
      System-wide install for Linux on Wayland · <code class="font-mono text-[12.5px] whitespace-nowrap text-fg-2">sh -s -- --user</code> for your home folder · or the
      <NuxtLink to="/#install" class="underline decoration-line-strong underline-offset-2 hover-motion hover:text-accent">AUR</NuxtLink>
    </p>
  </div>
</template>

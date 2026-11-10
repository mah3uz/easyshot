<script setup lang="ts">
const repo = 'https://github.com/mah3uz/easyshot'
const methods = {
  AUR: {
    note: 'Arch Linux. Two packages: easyshot builds from source, easyshot-bin is prebuilt. Install one.',
    code: '# with an AUR helper\nyay -S easyshot-bin\n# or build it from source\nyay -S easyshot',
  },
  'Other distros': {
    note: 'Installs the latest release for every user under /usr/local (through sudo or doas), after checking its checksum and, if minisign is installed, its signature, and starts it for you. Add <code>-s -- --user</code> after <code>sh</code> to install into ~/.local without sudo, or <code>-s -- --uninstall</code> to remove it.',
    code: 'curl -fsSL https://easyshot.site/install.sh | sh',
    manual: {
      label: 'Or install the tarball by hand',
      code: `# download it, SHA256SUMS and SHA256SUMS.minisig from ${repo}/releases/latest\nminisign -Vm SHA256SUMS -P RWRZb1Z2R8olADDs9ZWFvon7YercRduazJtfJoN3vltGVOU8I7zXuZE5\nsha256sum -c --ignore-missing SHA256SUMS\ntar xzf easyshot-*-x86_64-unknown-linux-gnu.tar.gz\ncd easyshot-*-x86_64-unknown-linux-gnu\n# for every user, or add --user for just you\nsudo ./install.sh`,
    },
  },
  Source: {
    note: 'Any distribution. Uses the Rust toolchain pinned in rust-toolchain.toml.',
    code: `git clone ${repo}\ncd easyshot\ncargo build --release -p easyshot\n# on Arch, build the package and install it with pacman\njust install`,
  },
} satisfies Record<string, { note: string; code: string; manual?: { label: string; code: string } }>

type Method = keyof typeof methods
const names = Object.keys(methods) as Method[]
const active = ref<Method>('AUR')

</script>

<template>
  <div class="grid gap-4">
    <TabPills v-model="active" :names="names" label="Install method" class="sm:w-fit" />
    <SwapPanel :swap-key="active">
      <div class="grid gap-3" role="tabpanel">
        <p class="install-note text-[14.5px] text-fg-2" v-html="methods[active].note" />
        <CodeBlock :code="methods[active].code" prompt />
        <template v-if="'manual' in methods[active]">
          <p class="mt-2 text-[13px] font-semibold text-fg-3">{{ methods[active].manual.label }}</p>
          <CodeBlock :code="methods[active].manual.code" prompt />
        </template>
      </div>
    </SwapPanel>
  </div>
</template>

<style scoped>
.install-note :deep(code) {
  border-radius: 6px;
  background: var(--surface-2);
  border: 1px solid var(--line);
  padding: 1px 6px;
  font-size: 13px;
  color: var(--fg);
}
</style>

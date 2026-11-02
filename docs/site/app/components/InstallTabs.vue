<script setup lang="ts">
// Install methods. Packages are published with the first release; source builds work today.
const { repo } = useRuntimeConfig().public

const methods = {
  AUR: {
    note: 'Arch Linux. Two packages: easyshot builds from source, easyshot-bin is prebuilt. Install one.',
    code: '# with an AUR helper\nyay -S easyshot-bin\n# or build it from source\nyay -S easyshot',
    ready: false,
  },
  Debian: {
    note: 'Debian, Ubuntu and derivatives. Download the .deb from the release page.',
    code: 'sudo apt install ./easyshot_<version>_amd64.deb',
    ready: false,
  },
  Fedora: {
    note: 'Fedora, openSUSE and other RPM distributions. Download the .rpm from the release page.',
    code: 'sudo dnf install ./easyshot-<version>.x86_64.rpm',
    ready: false,
  },
  Nix: {
    note: 'NixOS or Nix on any distribution.',
    code: '# try it once\nnix run github:mah3uz/easyshot\n# or install it to your profile\nnix profile install github:mah3uz/easyshot',
    ready: false,
  },
  Source: {
    note: 'Any distribution, today. Uses the Rust toolchain pinned in rust-toolchain.toml.',
    code: `git clone ${'https://github.com/mah3uz/easyshot'}\ncd easyshot\ncargo build --release -p easyshot\n# on Arch, build the package and install it with pacman\njust install`,
    ready: true,
  },
} as const

type Method = keyof typeof methods
const names = Object.keys(methods) as Method[]
const active = ref<Method>('AUR')
</script>

<template>
  <div class="grid gap-4">
    <div class="flex max-w-full flex-wrap gap-1 rounded-2xl border border-line bg-surface p-1 shadow-soft sm:w-fit sm:rounded-full" role="tablist" aria-label="Install method">
      <button
        v-for="n in names"
        :key="n"
        type="button"
        role="tab"
        :aria-selected="active === n"
        class="rounded-full px-4 py-1.5 text-sm font-medium transition-colors duration-300"
        :class="active === n ? 'bg-accent text-[#032830]' : 'text-fg-2 hover:text-fg'"
        @click="active = n"
      >
        {{ n }}
      </button>
    </div>
    <Transition mode="out-in" enter-active-class="transition duration-300 ease-out-soft" enter-from-class="opacity-0 translate-y-1" leave-active-class="transition duration-150" leave-to-class="opacity-0">
      <div :key="active" class="grid gap-3" role="tabpanel">
        <p class="flex flex-wrap items-center gap-2 text-[14.5px] text-fg-2">
          <span
            class="rounded-full px-2 py-0.5 text-xs font-semibold"
            :class="methods[active].ready ? 'bg-accent-soft text-accent' : 'bg-[#ffb36b]/15 text-[#d98030]'"
          >
            {{ methods[active].ready ? 'Works today' : 'With the first release' }}
          </span>
          {{ methods[active].note }}
        </p>
        <CodeBlock :code="methods[active].code" prompt />
        <p v-if="!methods[active].ready" class="text-[13px] text-fg-3">
          Not published yet. Until then, use <button type="button" class="text-accent underline" @click="active = 'Source'">Source</button>,
          or watch the <a :href="`${repo}/releases`" rel="noopener" class="text-accent underline">releases page</a>.
        </p>
      </div>
    </Transition>
  </div>
</template>

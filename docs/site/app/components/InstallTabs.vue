<script setup lang="ts">

const repo = 'https://github.com/mah3uz/easyshot'
const methods = {
  AUR: {
    note: 'Arch Linux. Two packages: easyshot builds from source, easyshot-bin is prebuilt. Install one.',
    code: '# with an AUR helper\nyay -S easyshot-bin\n# or build it from source\nyay -S easyshot',
  },
  'Other distros': {
    note: 'Any other distribution. The prebuilt tarball has an install script. Download the tarball, SHA256SUMS and SHA256SUMS.minisig from the latest release. It needs GStreamer and ffmpeg; see What it needs in the manual.',
    code: `# download from ${repo}/releases/latest, then verify\nminisign -Vm SHA256SUMS -P RWRZb1Z2R8olADDs9ZWFvon7YercRduazJtfJoN3vltGVOU8I7zXuZE5\nsha256sum -c --ignore-missing SHA256SUMS\ntar xzf easyshot-*-x86_64-unknown-linux-gnu.tar.gz\ncd easyshot-*-x86_64-unknown-linux-gnu\n# for every user\nsudo ./install.sh\n# or just for you\n./install.sh --user\n# remove it again\n./install.sh --uninstall`,
  },
  Source: {
    note: 'Any distribution. Uses the Rust toolchain pinned in rust-toolchain.toml.',
    code: `git clone ${repo}\ncd easyshot\ncargo build --release -p easyshot\n# on Arch, build the package and install it with pacman\njust install`,
  },
} as const

type Method = keyof typeof methods
const names = Object.keys(methods) as Method[]
const active = ref<Method>('AUR')

const buttons = ref<HTMLElement[]>([])
const pill = ref({ x: 0, y: 0, w: 0, h: 0 })
const measured = ref(false)
const animate = ref(false)

function measure() {
  const el = buttons.value[names.indexOf(active.value)]
  if (!el) return
  pill.value = { x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight }
  measured.value = true
}

onMounted(() => {
  measure()
  requestAnimationFrame(() => requestAnimationFrame(() => (animate.value = true)))
  window.addEventListener('resize', measure)
})
onBeforeUnmount(() => window.removeEventListener('resize', measure))
watch(active, () => nextTick(measure))
</script>

<template>
  <div class="grid gap-4">
    <div class="relative flex max-w-full flex-wrap gap-1 rounded-card border border-line bg-surface p-1 shadow-soft sm:w-fit sm:rounded-full" role="tablist" aria-label="Install method">
      <span
        v-if="measured"
        class="pointer-events-none absolute left-0 top-0 rounded-full bg-accent"
        :class="animate ? 'transition-[transform,width,height] duration-300 ease-out-soft' : ''"
        :style="{ width: pill.w + 'px', height: pill.h + 'px', transform: `translate(${pill.x}px, ${pill.y}px)` }"
        aria-hidden="true"
      />
      <button
        v-for="n in names"
        :key="n"
        :ref="(el) => { if (el) buttons[names.indexOf(n)] = el as HTMLElement }"
        type="button"
        role="tab"
        :aria-selected="active === n"
        class="relative rounded-full px-4 py-1.5 text-sm font-medium transition-colors duration-300 ease-out-soft"
        :class="[active === n ? 'text-[#032830]' : 'text-fg-2 hover:text-fg', active === n && !measured ? 'bg-accent' : '']"
        @click="active = n"
      >
        {{ n }}
      </button>
    </div>
    <Transition mode="out-in" enter-active-class="transition duration-300 ease-out-soft" enter-from-class="opacity-0 translate-y-1" leave-active-class="transition duration-150" leave-to-class="opacity-0">
      <div :key="active" class="grid gap-3" role="tabpanel">
        <p class="flex flex-wrap items-center gap-2 text-[14.5px] text-fg-2">
          {{ methods[active].note }}
        </p>
        <CodeBlock :code="methods[active].code" prompt />
      </div>
    </Transition>
  </div>
</template>

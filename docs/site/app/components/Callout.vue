<template>
  <div class="flex gap-3 rounded-2xl border border-accent/25 bg-accent-soft px-4 py-3.5 text-[14.5px] leading-relaxed text-fg-2" role="note">
    <Icon name="info" :size="18" class="mt-0.5 shrink-0 text-accent" />
    <div class="[&_code]:rounded-md [&_code]:bg-surface [&_code]:px-1.5 [&_code]:py-px [&_code]:text-fg"><slot /></div>
  </div>
</template>

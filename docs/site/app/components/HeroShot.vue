<template>
  <figure class="relative mx-auto w-full max-w-[1080px]">
    <div class="pointer-events-none absolute -inset-x-10 -top-10 bottom-0 -z-10 rounded-[60px] bg-[radial-gradient(ellipse_at_center,var(--glow),transparent_65%)] blur-2xl" />
    <RealShot shot="hero" sizes="(min-width: 1180px) 1080px, 100vw" eager />
    <div class="absolute -right-2 -bottom-6 w-[110px] sm:-right-6 sm:-bottom-8 sm:w-[160px]">
      <RealShot shot="deck" sizes="160px" />
    </div>
  </figure>
</template>

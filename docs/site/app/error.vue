<script setup lang="ts">
import type { NuxtError } from '#app'
const props = defineProps<{ error: NuxtError }>()
useSeoMeta({ title: props.error.statusCode === 404 ? 'Page not found' : 'Something went wrong', robots: 'noindex' })
</script>

<template>
  <div class="flex min-h-dvh flex-col">
    <SiteHeader />
    <main class="container-x grid flex-1 place-items-center py-40 text-center">
      <div class="grid justify-items-center gap-5">
        <img src="/logo.svg" alt="" width="72" height="72" class="animate-float" />
        <p class="eyebrow">Error {{ error.statusCode }}</p>
        <h1 class="text-4xl sm:text-5xl">{{ error.statusCode === 404 ? 'Nothing was captured here.' : 'Something went wrong.' }}</h1>
        <p class="max-w-md text-fg-2">The page you asked for does not exist, or has moved.</p>
        <button type="button" class="btn btn-primary" @click="clearError({ redirect: '/' })">Back to the home page</button>
      </div>
    </main>
    <SiteFooter />
  </div>
</template>

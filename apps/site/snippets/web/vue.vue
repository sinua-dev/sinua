<!-- vite.config: vue({ template: { compilerOptions: { isCustomElement: (tag) => tag === "sinua-view" } } })
     main.ts:     import "@sinua/web/element";   Types: import type {} from "@sinua/web/types/vue"; -->
<script setup lang="ts">
import { ref } from "vue";
import voiceOrb from "./voice-orb.fxspec.json";

const state = ref("listening");
const muted = ref(false);
</script>

<template>
  <!-- Objects bind as properties (:spec, :inputs); events with @. -->
  <sinua-view :spec="voiceOrb" :state="state" :inputs="{ micMuted: muted ? 1 : 0 }" @fxerror="console.warn($event.detail)" style="width: 160px" />
</template>

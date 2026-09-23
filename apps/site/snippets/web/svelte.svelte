<!-- No setup beyond registering the tag. Types: import type {} from "@sinua/web/types/svelte"; -->
<script lang="ts">
  import "@sinua/web/element";
  import voiceOrb from "./voice-orb.fxspec.json";

  let { state = "listening", muted = false } = $props();
</script>

<!-- Svelte sets properties the element has, so objects pass straight through. -->
<sinua-view spec={voiceOrb} {state} inputs={{ micMuted: muted ? 1 : 0 }} onfxerror={(e: CustomEvent<unknown[]>) => console.warn(e.detail)} style="width: 160px"></sinua-view>

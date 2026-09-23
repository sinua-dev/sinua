<script setup lang="ts">
import { ref } from "vue";
import spec from "../../spec.json";
const inputs = ref({ micMuted: 0 });
const frames = ref(0);
const shown = ref(true);
</script>

<template>
  <!-- Vue sets properties when the element has them (`in` check); objects bind as properties. -->
  <sinua-view v-if="shown" id="fx" style="width: 192px" theme="light" :spec="spec" :inputs="inputs" @fxframe="frames++" />
  <p>frames: <span id="frames">{{ frames }}</span></p>
  <button id="toggle" @click="inputs = { micMuted: inputs.micMuted ? 0 : 1 }">toggle micMuted</button>
  <button id="remove" @click="shown = false">remove</button>
</template>

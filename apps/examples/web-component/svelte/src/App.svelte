<script lang="ts">
  import spec from "../../spec.json";
  let inputs = $state({ micMuted: 0 });
  let frames = $state(0);
  let shown = $state(true);
</script>

{#if shown}
  <sinua-view id="fx" style="width: 192px" theme="light" {spec} {inputs} onfxframe={() => frames++}></sinua-view>
{/if}
<p>frames: <span id="frames">{frames}</span></p>
<button id="toggle" onclick={() => (inputs = { micMuted: inputs.micMuted ? 0 : 1 })}>toggle micMuted</button>
<button id="remove" onclick={() => (shown = false)}>remove</button>

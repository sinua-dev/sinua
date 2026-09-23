import "@sinua/web/element";
import type {} from "@sinua/web/types/solid";
import { render } from "solid-js/web";
import { createSignal, Show } from "solid-js";
import spec from "../../spec.json";

function App() {
  const [inputs, setInputs] = createSignal({ micMuted: 0 });
  const [frames, setFrames] = createSignal(0);
  const [shown, setShown] = createSignal(true);
  return (
    <>
      <Show when={shown()}>
        <sinua-view id="fx" style={{ width: "192px" }} theme="light" prop:spec={spec} prop:inputs={inputs()} on:fxframe={() => setFrames(frames() + 1)} />
      </Show>
      <p>frames: <span id="frames">{frames()}</span></p>
      <button id="toggle" onClick={() => setInputs({ micMuted: inputs().micMuted ? 0 : 1 })}>toggle micMuted</button>
      <button id="remove" onClick={() => setShown(false)}>remove</button>
    </>
  );
}
render(() => <App />, document.getElementById("app")!);

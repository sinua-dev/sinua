import { mount } from "@sinua/web";

const canvas = document.querySelector<HTMLCanvasElement>("#coach")!;
const fx = mount(canvas, { pattern: "tracking", label: "Coach" });

// One short effect on top of whatever the view shows, then it's exactly as before.
export function onGoalReached() {
  fx.trigger("celebrate"); // or "success", "error"; <sinua-view>: el.trigger("celebrate")
}

// The words spoken with it can be yours (default: "Done", "Something went wrong", "Well done").
fx.update({ labels: { "effect:celebrate": "Goal reached" } });

import "@sinua/web/element";
import spec from "../spec.json";
const el = document.getElementById("fx");
el.spec = spec;
el.inputs = { micMuted: 0 };
let n = 0;
el.addEventListener("fxframe", () => { document.getElementById("frames").textContent = String(++n); });
document.getElementById("toggle").onclick = () => { el.inputs = { micMuted: el.inputs.micMuted ? 0 : 1 }; };
document.getElementById("remove").onclick = () => el.remove();

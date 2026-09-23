import { parameterCatalog } from "@sinua/core";

const catalog = parameterCatalog();
const orb = catalog.objects.find((o) => o.id === "orb")!;
const breathing = orb.patterns.find((p) => p.id === "breathing")!;

for (const { ref, default: byPattern } of breathing.params) {
  const d = catalog.definitions[ref];
  console.log(d.path, d.type, `${d.min}–${d.max}`, "default at 64:", byPattern["64"], "—", d.description);
}

import { characterRecipe, resolveFxSpec } from "@sinua/core";
import { fxSpecCost } from "@sinua/core/dev"; // tools and checks; your app doesn't ship it
import { fitPath, pathBox, svgPaths } from "@sinua/snippets";

declare const svgFileText: string; // an SVG exported from your drawing tool

// 1. Start from a built-in: its recipe, as the engine carries it.
const cuppa = characterRecipe("cuppa")!;

// 4. A body from a drawing: every <path> in the file, arcs turned into curves,
//    fitted into the old body's box so the face, steam and rig stay where they were.
const oldBody = "M52 48 C52 40 128 40 128 48 L122 166 C121 174 114 178 106 178 H74 C66 178 59 174 58 166 Z";
const { d } = svgPaths(svgFileText);
const fitted = fitPath(d, pathBox(oldBody) ?? [52, 40, 128, 178]);
if (fitted.warnings.length) console.warn(fitted.warnings);

// 2.–5. A new id, the original's behaviour, your changes; then one FX Spec file.
const latte = {
  $schema: "https://sinua.dev/schema/fx-spec-1.json",
  fxSpec: "1.12",
  object: "character",
  pattern: "latte",
  recipe: { ...cuppa, id: "latte", profile: "cuppa" /* palette, parts, the body's shape: fitted.d */ },
};

// 6. Check it: mistakes come back with a pointer into the recipe, and the cost class.
const r = resolveFxSpec(JSON.stringify(latte), { state: "speaking" });
if (!r.ok) console.error(r.diagnostics); // e.g. /recipe/parts/2/rise: expected a number
console.log(fxSpecCost(JSON.stringify(latte))?.class); // "light"

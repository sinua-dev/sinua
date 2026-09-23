import { checkOverrides } from "@sinua/core";

// Warnings only: the frame still renders, and the engine clamps out-of-range values.
const warnings = checkOverrides("breathing", 64, { lanse: 6 });
for (const w of warnings) {
  console.warn(`${w.path}: ${w.message}`);
  // → /lanse: unknown key `lanse` for orb/breathing (did you mean `lanes`?)
}

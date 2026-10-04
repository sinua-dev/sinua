/**
 * Code tabs for one design, per platform, from the Studio's own exporter
 * (@sinua/snippets): the typed component where the exporter has one (React,
 * React Native), the generic view elsewhere (JavaScript, SwiftUI, Compose).
 */
import { parameterCatalog } from "./catalog";
import { buildSnippets, buildTypedSnippets, toTypedProps } from "@sinua/snippets";
import type { SnippetTab } from "@sinua/design";

export type TabId = "typed-react" | "web" | "swiftui" | "compose" | "typed-rn";

const LABEL: Record<TabId, string> = {
  "typed-react": "React",
  web: "JavaScript",
  swiftui: "SwiftUI",
  compose: "Compose",
  "typed-rn": "React Native",
};

export function designTabs(object: string, pattern: string, overrides: Record<string, number>, ids: TabId[]): SnippetTab[] {
  const typed = buildTypedSnippets({ design: toTypedProps(object, pattern, overrides, parameterCatalog()), size: 64 });
  const view = buildSnippets({ state: pattern, size: 64, overrides, specFile: `${object}-${pattern}.fxspec.json` });
  const byId = new Map<string, string>([
    ...typed.map((t) => [`typed-${t.id}`, t.code] as const),
    // The generic view's text ends with a pointer to the Studio's file export; not here.
    ...view.code.map((t) => [t.id, t.code.split("\n\n// Or use the file")[0]] as const),
  ]);
  return ids.filter((id) => byId.has(id)).map((id) => ({ id, label: LABEL[id], code: byId.get(id)! }));
}

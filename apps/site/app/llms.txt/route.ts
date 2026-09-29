/**
 * /llms.txt (llmstxt.org): what Sinua is and where every page is, for a coding
 * agent that reads the site before writing code. Built once at export time.
 */
import { brand } from "@/lib/brand";
import { VERSION } from "@/lib/prompt";
import { source } from "@/lib/source";
import { parameterCatalog } from "@sinua/core";

export const dynamic = "force-static";

// Counted from the engine's catalog, so a new pattern or family can't leave this stale.
const CATALOG = parameterCatalog();
const OBJECTS = CATALOG.objects.map((o) => o.id);
const PATTERNS = CATALOG.objects.reduce((n, o) => n + o.patterns.length, 0);

export function GET() {
  const url = (path: string) => new URL(path.endsWith("/") ? path : `${path}/`, brand.siteUrl).toString();
  const sections = new Map<string, string[]>();
  for (const page of source.getPages()) {
    const section = page.slugs.length > 1 ? page.slugs[0] : "overview";
    const line = `- [${page.data.title}](${url(page.url)})${page.data.description ? `: ${page.data.description}` : ""}`;
    sections.set(section, [...(sections.get(section) ?? []), line]);
  }
  const title = (s: string) => s.replace(/-/g, " ").replace(/^./, (c) => c.toUpperCase());
  const text = [
    `# ${brand.name}`,
    ``,
    `> ${brand.description} Public beta (${VERSION}); the API can still change between betas.`,
    ``,
    `A design is a pattern (${PATTERNS}, in ${OBJECTS.length} families: ${OBJECTS.join(", ")}) plus props, or one FX Spec file (.fxspec.json, 1.9) that every platform reads the same way. Views take an agent state (idle, listening, thinking, speaking) or a voice source.`,
    ``,
    `Install:`,
    `- Web: npm i @sinua/web@beta @sinua/core@beta (npm's untagged name still resolves to the first beta)`,
    `- iOS (SwiftPM): https://github.com/sinua-dev/sinua-swift from "${VERSION}", product "Sinua" (voice: "SinuaVoice")`,
    `- Android (Gradle): dev.sinua:sinua-view:${VERSION} (+ dev.sinua:sinua-core)`,
    `- React Native: build from source for now`,
    ``,
    `The whole documentation as one file: ${url("/llms-full.txt").replace(/\/$/, "")}`,
    `Every pattern live, with code: ${url(brand.links.gallery)}`,
    `Source (Apache-2.0): ${brand.links.repo}`,
    ``,
    ...[...sections].flatMap(([s, lines]) => [`## ${title(s)}`, ``, ...lines, ``]),
  ].join("\n");
  return new Response(text, { headers: { "Content-Type": "text/plain; charset=utf-8" } });
}

/**
 * /llms-full.txt: every docs page as processed Markdown, in the site's order,
 * with the included code samples inlined, so an agent can read the docs in one
 * request. Built once at export time.
 */
import { brand } from "@/lib/brand";
import { source } from "@/lib/source";

export const dynamic = "force-static";

export async function GET() {
  const pages = await Promise.all(
    source.getPages().map(async (page) => {
      const body = await page.data.getText("processed");
      return `# ${page.data.title}\n\nSource: ${new URL(`${page.url}/`, brand.siteUrl).toString()}\n\n${page.data.description ?? ""}\n\n${body}`;
    })
  );
  return new Response(pages.join("\n\n---\n\n") + "\n", { headers: { "Content-Type": "text/plain; charset=utf-8" } });
}

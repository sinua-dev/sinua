import { loader } from "fumadocs-core/source";
import { defineDocs } from "fumadocs-mdx/macro";
import { metaSchema, pageSchema } from "fumadocs-core/source/schema";
import { z } from "zod";
import { docsRoute } from "./shared";

// Pages: content/docs (families). content/generated holds partials pulled in
// with <include>, not pages; snippets/ holds included code files.
const docs = defineDocs({
  dir: "content/docs",
  docs: {
    // `metaTitle`: the search-result title where it should say more than the nav does
    // ("LiveKit" in the sidebar, "LiveKit agent visualizer for SwiftUI, Compose and React" in search).
    schema: pageSchema.extend({ metaTitle: z.string().optional() }),
    // Each page as processed Markdown (includes resolved to their code), for /llms-full.txt.
    postprocess: { includeProcessedMarkdown: true },
  },
  meta: { schema: metaSchema },
});

export const source = loader({
  baseUrl: docsRoute,
  source: docs.toFumadocsSource(),
});

// Terms, privacy and refunds (content/legal): plain pages under /legal/, in the
// landing's chrome, not the docs. `updated` is the date shown under the title.
const legal = defineDocs({
  dir: "content/legal",
  docs: { schema: pageSchema.extend({ updated: z.string() }) },
  meta: { schema: metaSchema },
});

export const legalSource = loader({
  baseUrl: "/legal",
  source: legal.toFumadocsSource(),
});

// The site's changelog page is the repository's CHANGELOG.md, so the two can't disagree:
// the released versions (from the first `## <version>` heading on) are copied into a
// partial the page includes. Run before `dev` and `build`, like copy-specs.mjs; the
// partial is generated, not checked in.
import { readFileSync, writeFileSync } from "node:fs";

const REPO = "https://github.com/sinua-dev/sinua/blob/main/";
const text = readFileSync(new URL("../../../CHANGELOG.md", import.meta.url), "utf8");
const start = text.search(/^## \d/m);
if (start < 0) throw new Error("CHANGELOG.md: no `## <version>` heading to publish");
const body = text
  .slice(start)
  // Repository-relative links point at GitHub; the site has no copy of those files.
  .replace(/\]\((?!https?:|#)([^)]+)\)/g, (_, path) => `](${REPO}${path})`);
writeFileSync(new URL("../content/changelog.generated.mdx", import.meta.url), body);

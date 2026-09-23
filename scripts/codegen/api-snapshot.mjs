#!/usr/bin/env node
// The generated components' public contract, frozen: names and types, not bytes.
//   node scripts/codegen/api-snapshot.mjs           check the generated files against api-snapshot.json
//   node scripts/codegen/api-snapshot.mjs --update  rewrite api-snapshot.json (review the diff: it is an API change)
//
// Why a second gate next to `generate.mjs --check`: that one pins the generated
// files' *bytes* to the catalog, so a catalog edit plus a regeneration passes it
// by construction. Renaming a public parameter in spec/parameters.json and
// regenerating left every gate green while seven generated files renamed a prop
// across Swift, Kotlin, two TS packages and the web components
// (a pre-launch review's change 2). This file records what a caller can
// name -- declarations, members, parameter lists, pattern ids -- so that edit
// fails here until someone runs `--update` on purpose. `generate.mjs` never
// writes the snapshot: a gate whose remedy is regeneration launders the change.
//
// Formatting-insensitive by design: comments, whitespace, trailing commas and
// default values are dropped. Member lists are compared as sets; a parameter
// list stays ordered inside its signature, because Kotlin constructors/funs and
// Swift inits are called positionally or label-ordered.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join, resolve, extname, basename, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { render } from "./generate.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const SNAPSHOT = join(ROOT, "scripts/codegen/api-snapshot.json");

/** "swift" | "kotlin" | "ts" from a generated file's path. */
function langOf(path) {
  const ext = extname(path);
  if (ext === ".swift") return "swift";
  if (ext === ".kt") return "kotlin";
  if (ext === ".ts" || ext === ".tsx") return "ts";
  throw new Error(`api-snapshot: no extractor for ${path}`);
}

const QUOTES = { swift: ['"'], kotlin: ['"', "'"], ts: ['"', "'", "`"] };

/** Index just past the string literal starting at i (escapes honoured; Swift/Kotlin triple quotes too). */
function skipString(text, i) {
  const q = text[i];
  if (q === '"' && text.startsWith('"""', i)) {
    const end = text.indexOf('"""', i + 3);
    return end < 0 ? text.length : end + 3;
  }
  for (let j = i + 1; j < text.length; j++) {
    if (text[j] === "\\") j++;
    else if (text[j] === q) return j + 1;
  }
  return text.length;
}

/** Comments out, string literals kept intact (a "//" inside a string is not a comment). */
export function stripComments(text, lang) {
  let out = "";
  for (let i = 0; i < text.length; ) {
    if (text.startsWith("//", i)) {
      const nl = text.indexOf("\n", i);
      i = nl < 0 ? text.length : nl;
    } else if (text.startsWith("/*", i)) {
      const end = text.indexOf("*/", i + 2);
      i = end < 0 ? text.length : end + 2;
      out += " ";
    } else if (QUOTES[lang].includes(text[i])) {
      const j = skipString(text, i);
      out += text.slice(i, j);
      i = j;
    } else out += text[i++];
  }
  return out;
}

/** Index of the bracket closing the one at `open`, skipping strings and nested brackets. */
function matching(text, open, lang) {
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    const c = text[i];
    if (QUOTES[lang].includes(c)) {
      i = skipString(text, i) - 1;
      continue;
    }
    if (c === "(" || c === "[" || c === "{") depth++;
    else if (c === ")" || c === "]" || c === "}") {
      depth--;
      if (depth === 0) return i;
    }
  }
  throw new Error(`api-snapshot: unbalanced bracket at ${open}`);
}

/** Split at `sep` characters that sit at bracket depth 0. */
function splitTop(text, sep, lang) {
  const parts = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (QUOTES[lang].includes(c)) {
      i = skipString(text, i) - 1;
      continue;
    }
    if ("([{<".includes(c)) depth++;
    else if (")]}".includes(c) || (c === ">" && text[i - 1] !== "-" && text[i - 1] !== "=")) depth--;
    else if (depth === 0 && sep.includes(c)) {
      parts.push(text.slice(start, i));
      start = i + 1;
    }
  }
  parts.push(text.slice(start));
  return parts;
}

/** Whitespace collapsed, and dropped next to punctuation, so re-flowing a line changes nothing. */
export function norm(s) {
  return s
    .replace(/\s+/g, " ")
    .trim()
    .replace(/ ?([()[\]{}<>,;:|&=?!]) ?/g, "$1")
    .replace(/,([)\]}])/g, "$1")
    .replace(/;([)\]}])/g, "$1")
    .replace(/[,;]$/, "");
}

/** Index of a top-level `=` that starts a default/initializer (not ==, =>, !=, <=, >=). */
function initializerAt(text, lang) {
  let depth = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (QUOTES[lang].includes(c)) {
      i = skipString(text, i) - 1;
      continue;
    }
    if ("([{<".includes(c)) depth++;
    else if (")]}".includes(c) || (c === ">" && text[i - 1] !== "-" && text[i - 1] !== "=")) depth--;
    // Neighbours default to a space: `"=>".includes("")` is true, which once hid a trailing `=`.
    else if (c === "=" && depth === 0 && !"=!<>".includes(text[i - 1] || " ") && !"=>".includes(text[i + 1] || " ")) return i;
  }
  return -1;
}

const dropInitializer = (s, lang) => {
  const at = initializerAt(s, lang);
  return at < 0 ? s : s.slice(0, at);
};

/** The first parameter list in a signature, with each parameter's default value removed. */
function dropParamDefaults(sig, lang) {
  const open = sig.indexOf("(");
  if (open < 0) return sig;
  const close = matching(sig, open, lang);
  const params = splitTop(sig.slice(open + 1, close), ",", lang)
    .map((p) => dropInitializer(p, lang).trim())
    .filter(Boolean);
  return `${sig.slice(0, open)}(${params.join(", ")})${sig.slice(close + 1)}`;
}

/**
 * Depth-0 statements of `text`: `{ header, body }` where body is the text of a
 * `{...}` block that followed the header (or null). Swift/Kotlin statements end
 * at a newline or `;`, TS ones at `;`. In TS a `{` after `type`/`const` is part
 * of the value, not a block.
 */
function statements(text, lang) {
  const out = [];
  let depth = 0;
  let start = 0;
  const flush = (end, body = null) => {
    const header = text.slice(start, end);
    if (header.trim() || body !== null) out.push({ header, body });
  };
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (QUOTES[lang].includes(c)) {
      i = skipString(text, i) - 1;
      continue;
    }
    if (c === "{" && depth === 0) {
      const head = text.slice(start, i);
      // `export { A, type B } from "..."` and `import { ... }` are lists, not blocks.
      const valueBrace =
        lang === "ts" && (/^\s*(export\s+)?(declare\s+)?(type|const|let|var)\b/.test(head) || /^\s*(export|import)(\s+type)?\s*$/.test(head));
      const kotlinInit = lang !== "ts" && /=\s*$/.test(head); // `val x = { ... }`, a lambda value
      if (!valueBrace && !kotlinInit) {
        const close = matching(text, i, lang);
        flush(i, text.slice(i + 1, close));
        i = close;
        start = i + 1;
        continue;
      }
    }
    if ("([{".includes(c)) depth++;
    else if (")]}".includes(c)) depth--;
    else if (depth === 0 && (c === ";" || (c === "\n" && lang !== "ts"))) {
      flush(i);
      start = i + 1;
    }
  }
  flush(text.length);
  return out;
}

const TYPE_RE = {
  swift: /^((?:(?:public|open|final|indirect|@\w+)\s+)*)(struct|enum|class|protocol|extension)\s+([\w.]+)/,
  kotlin: /^((?:(?:public|private|internal|protected|data|enum|sealed|abstract|open|annotation|inner|value|@\w+(?:\([^)]*\))?)\s+)*)(class|interface|object|companion\s+object)\b\s*(\w*)/,
};

/** Swift / Kotlin: one type or scope's members into `decls`. */
function walkNative(text, lang, scope, decls) {
  const top = scope || "(top level)";
  const add = (key, sig) => (decls[key] ??= []).push(sig);
  // An annotation on its own line (`@Composable`, `@MainActor`) belongs to the
  // next declaration, and is part of its contract: dropping it would miss a
  // composable becoming a plain function.
  let pending = "";
  for (const { header, body } of statements(text, lang)) {
    let h = header.trim();
    if (!h) continue;
    if (body == null && /^(@\w+(\([^)]*\))?\s*)+$/.test(h)) {
      pending += `${h} `;
      continue;
    }
    h = pending + h;
    pending = "";
    const t = TYPE_RE[lang].exec(h);
    if (t) {
      const mods = t[1];
      const kind = t[2].replace(/\s+/g, " ");
      const name = t[3] || (kind === "companion object" ? "Companion" : "");
      if (lang === "swift" && kind !== "extension" && !/\bpublic\b|\bopen\b/.test(mods)) continue;
      if (lang === "kotlin" && /\b(private|internal|protected)\b/.test(mods)) continue;
      const key = scope ? `${scope}.${name}` : name;
      add(key, `decl ${norm(lang === "kotlin" ? dropParamDefaults(h, lang) : h)}`);
      if (body == null) continue;
      if (lang === "kotlin" && /\benum\b/.test(mods)) {
        // Entries first, up to a top-level `;`; members (if any) after it.
        const [entries, ...rest] = splitTop(body, ";", lang);
        for (const e of splitTop(entries, ",", lang)) if (e.trim()) add(key, `entry ${norm(e)}`);
        walkNative(rest.join(";"), lang, key, decls);
      } else walkNative(body, lang, key, decls);
      continue;
    }
    if (lang === "swift") {
      if (/^case\b/.test(h)) add(top, norm(h));
      else if (/\b(public|open)\b/.test(h) && /\b(var|let|init|func|subscript|typealias|static)\b/.test(h)) {
        const sig = /\b(init|func|subscript)\b/.test(h) ? dropParamDefaults(h, lang) : dropInitializer(h, lang);
        add(top, norm(sig));
      }
    } else {
      if (/\b(private|internal|protected)\b/.test(h)) continue;
      const m = /^((?:@\w+(?:\([^)]*\))?\s+|(?:public|override|inline|operator|infix|suspend|const|lateinit|vararg)\s+)*)(fun|val|var|typealias)\b/.exec(h);
      if (!m) continue; // a continuation line or an expression body; not a declaration
      const sig = m[2] === "fun" ? dropInitializer(dropParamDefaults(h, lang), lang) : m[2] === "typealias" ? h : dropInitializer(h, lang);
      add(top, norm(sig));
    }
  }
}

/** TS / TSX: exported declarations (plus local `type` aliases, which exported ones name). */
function walkTs(text, scope, decls) {
  const top = scope || "(top level)";
  const add = (key, sig) => (decls[key] ??= []).push(sig);
  for (const { header, body } of statements(text, "ts")) {
    const h = header.trim();
    if (!h || /^import\b/.test(h)) continue;
    const iface = /^(?:export\s+)?(?:declare\s+)?interface\s+(\w+)/.exec(h);
    if (iface && body != null) {
      const key = scope ? `${scope}.${iface[1]}` : iface[1];
      add(key, `decl ${norm(h)}`);
      for (const m of splitTop(body, ";\n", "ts")) if (m.trim()) add(key, norm(m));
      continue;
    }
    if (/^declare\s+global\b/.test(h) && body != null) {
      walkTs(body, scope ? `${scope}.global` : "global", decls);
      continue;
    }
    const exported = /^export\b/.test(h);
    if (/^(?:export\s+)?type\s+\w+/.test(h)) add(top, norm(h));
    else if (!exported) continue;
    else if (/^export\s+(?:async\s+)?function\b/.test(h)) add(top, norm(dropParamDefaults(h, "ts")));
    else if (/^export\s+(?:const|let|var)\b/.test(h)) add(top, norm(dropInitializer(h, "ts")));
    else if (/^export\s*\{/.test(h)) {
      const inner = h.slice(h.indexOf("{") + 1, h.lastIndexOf("}"));
      const names = inner.split(",").map((s) => norm(s)).filter(Boolean).sort();
      // The names are the contract; the `from "./X.js"` specifier is the package's internal layout.
      add(top, `export {${names.join(",")}}`);
    } else add(top, norm(h));
  }
}

/** The web components' tag table: per tag, its object, attribute params and property groups. */
function elementTags(text, decls) {
  const re = /"(sinua-[a-z-]+)"\s*:\s*\{\s*object:\s*"(\w+)",\s*params:\s*(\[[^\]]*\]),\s*groups:\s*(\[[^\]]*\])/g;
  let n = 0;
  for (const m of text.matchAll(re)) {
    const key = `tag ${m[1]}`;
    const list = (decls[key] ??= []);
    list.push(`object ${m[2]}`);
    for (const p of JSON.parse(m[3])) list.push(`param ${p}`);
    for (const g of JSON.parse(m[4])) list.push(`group ${g}`);
    n++;
  }
  if (n === 0) throw new Error("api-snapshot: SinuaElements.ts has no tag entries -- the extractor no longer matches it");
}

/**
 * `{ relPath: { declaration: sortedMembers[] } }` for generated files given as
 * `Map(relPath -> contents)`. Throws when a file yields nothing: an extractor
 * that stops matching must fail, not snapshot an empty set.
 */
export function extractContract(files) {
  const contract = {};
  for (const [path, contents] of [...files].sort(([a], [b]) => a.localeCompare(b))) {
    const lang = langOf(path);
    const text = stripComments(contents, lang);
    const decls = {};
    if (lang === "ts") walkTs(text, "", decls);
    else walkNative(text, lang, "", decls);
    if (basename(path) === "SinuaElements.ts") elementTags(text, decls);
    const keys = Object.keys(decls);
    if (keys.length === 0) throw new Error(`api-snapshot: no public declarations found in ${path} -- the extractor no longer matches it`);
    contract[path] = Object.fromEntries(keys.sort().map((k) => [k, [...new Set(decls[k])].sort()]));
  }
  return contract;
}

/** Counts for the success line: files, declarations, members. */
export function census(contract) {
  const files = Object.keys(contract).length;
  let decls = 0;
  let members = 0;
  for (const f of Object.values(contract))
    for (const m of Object.values(f)) {
      decls++;
      members += m.length;
    }
  return { files, decls, members };
}

/** Human-readable differences, one entry per file/declaration that changed. */
export function diffContracts(expected, actual) {
  const out = [];
  for (const file of new Set([...Object.keys(expected), ...Object.keys(actual)])) {
    const e = expected[file];
    const a = actual[file];
    if (!e) {
      out.push({ file, decl: "(file)", added: ["the whole file is new to the snapshot"], removed: [] });
      continue;
    }
    if (!a) {
      out.push({ file, decl: "(file)", added: [], removed: ["the whole file is gone"] });
      continue;
    }
    for (const decl of new Set([...Object.keys(e), ...Object.keys(a)])) {
      const em = new Set(e[decl] ?? []);
      const am = new Set(a[decl] ?? []);
      const removed = [...em].filter((m) => !am.has(m));
      const added = [...am].filter((m) => !em.has(m));
      if (removed.length || added.length) out.push({ file, decl, removed, added });
    }
  }
  return out;
}

/** Relative path -> contents for every generated file, read from disk (generate.mjs --check pins disk == render). */
export function generatedFiles(root = ROOT) {
  const { files } = render(root);
  const map = new Map();
  for (const abs of files.keys()) {
    const rel = relative(root, abs);
    if (!existsSync(abs)) throw new Error(`api-snapshot: ${rel} is missing -- run node scripts/codegen/generate.mjs`);
    map.set(rel, readFileSync(abs, "utf8"));
  }
  return map;
}

export function loadSnapshot(path = SNAPSHOT) {
  return JSON.parse(readFileSync(path, "utf8")).files;
}

function write(contract, path = SNAPSHOT) {
  const doc = {
    note:
      "The generated components' public contract (scripts/codegen/api-snapshot.mjs). An API change: " +
      "update with `node scripts/codegen/api-snapshot.mjs --update` and review this diff deliberately. " +
      "generate.mjs never writes it.",
    files: contract,
  };
  writeFileSync(path, `${JSON.stringify(doc, null, 2)}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const files = generatedFiles();
  const actual = extractContract(files);
  const { files: nf, decls, members } = census(actual);
  if (nf !== files.size) throw new Error(`api-snapshot: ${files.size} generated files but ${nf} in the contract`);
  if (process.argv.includes("--update")) {
    write(actual);
    console.log(`api snapshot written: ${nf} file(s), ${decls} declaration(s), ${members} member(s)`);
  } else {
    const diffs = diffContracts(loadSnapshot(), actual);
    if (diffs.length) {
      console.log("api snapshot: the generated public API changed");
      for (const d of diffs) {
        console.log(`  ${d.file} :: ${d.decl}`);
        for (const r of d.removed) console.log(`    - ${r}`);
        for (const a of d.added) console.log(`    + ${a}`);
      }
      console.log("If this is intended: node scripts/codegen/api-snapshot.mjs --update, and review the JSON diff.");
      process.exit(1);
    }
    console.log(`api snapshot: ${nf} file(s), ${decls} declaration(s), ${members} member(s) match`);
  }
}

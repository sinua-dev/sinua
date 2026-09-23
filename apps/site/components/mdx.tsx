import defaultMdxComponents from "fumadocs-ui/mdx";
import { Tab, Tabs } from "fumadocs-ui/components/tabs";
import type { MDXComponents } from "mdx/types";
import { Demo } from "@/components/demo";
import { GalleryLink } from "@/components/gallery-link";

/** Everything the content uses: the defaults (Callout, Cards/Card, code blocks…) + Tabs/Tab + Demo. */
export function getMDXComponents(components?: MDXComponents) {
  return {
    ...defaultMdxComponents,
    Tabs,
    Tab,
    Demo,
    GalleryLink,
    ...components,
  } satisfies MDXComponents;
}

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>;
}

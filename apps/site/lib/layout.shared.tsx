import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";
import { brand } from "./brand";
import { appName } from "./shared";

export function baseOptions(): BaseLayoutProps {
  return {
    nav: { title: appName },
    githubUrl: brand.links.repo,
    links: [
      { text: "Gallery", url: brand.links.gallery },
      { text: "Studio", url: brand.links.studioPage },
    ],
  };
}

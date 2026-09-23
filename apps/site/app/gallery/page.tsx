import type { Metadata } from "next";
import { brand } from "@/lib/brand";
import { Gallery } from "@/components/gallery";

export const metadata: Metadata = {
  title: "Pattern gallery",
  description: `Every ${brand.name} pattern, live, in the four voice states, with its parameters and the code to paste.`,
};

export default function GalleryPage() {
  return <Gallery />;
}

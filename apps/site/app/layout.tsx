import type { Metadata } from "next";
import { brand } from "@/lib/brand";
import { Provider } from "@/components/provider";
import "./global.css";

export const metadata: Metadata = {
  metadataBase: new URL(brand.siteUrl),
  title: { template: `%s · ${brand.name}`, default: `${brand.name}: ${brand.copy.tagline}` },
  description: brand.description,
  openGraph: { type: "website", siteName: brand.name, url: "/", description: brand.description },
  twitter: { card: "summary", description: brand.description },
};

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <Provider>{children}</Provider>
      </body>
    </html>
  );
}

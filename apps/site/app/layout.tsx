import type { Metadata } from "next";
import { brand } from "@/lib/brand";
import { Provider } from "@/components/provider";
import "./global.css";

export const metadata: Metadata = {
  title: { template: `%s · ${brand.name}`, default: brand.name },
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

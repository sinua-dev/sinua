"use client";
import SearchDialog from "@/components/search";
import { RootProvider } from "fumadocs-ui/provider/next";
import { type ReactNode } from "react";

export function Provider({ children }: { children: ReactNode }) {
  // Dark first: the visuals read best on a dark ground. The toggle still switches, and
  // remembers the choice.
  return (
    <RootProvider search={{ SearchDialog }} theme={{ defaultTheme: "dark" }}>
      {children}
    </RootProvider>
  );
}

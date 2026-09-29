"use client";
/**
 * Copy buttons for a design: the whole prompt for a coding agent, or just the FX
 * Spec file. Both are built from the same file (lib/spec-file), so what an agent
 * gets and what a person downloads never differ.
 */
import type { OrbSize } from "@sinua/core";
import { Icon, useCopy } from "@sinua/design";
import { designPrompt } from "@/lib/prompt";
import { specFile, specText } from "@/lib/spec-file";

interface Design {
  object: string;
  pattern: string;
  /** The pattern's label, for the prompt's first line. */
  label?: string;
  size?: OrbSize;
  /** Only the values someone changed; the rest are the pattern's defaults. */
  changed?: Record<string, number>;
}

export function CopyPrompt({ object, pattern, label, size = 64, changed = {} }: Design) {
  const [copied, copy] = useCopy();
  return (
    <button type="button" className="tb-btn copy-btn" onClick={() => copy(designPrompt(object, pattern, label ?? pattern, size, changed))}>
      <Icon name={copied ? "check" : "copy"} />
      {copied ? "Copied" : "Copy prompt"}
    </button>
  );
}

export function CopySpec({ object, pattern, size = 64, changed = {} }: Design) {
  const [copied, copy] = useCopy();
  return (
    <button type="button" className="tb-btn copy-btn" onClick={() => copy(specText(specFile(object, pattern, size, changed)))}>
      <Icon name={copied ? "check" : "copy"} />
      {copied ? "Copied" : "Copy FX Spec"}
    </button>
  );
}

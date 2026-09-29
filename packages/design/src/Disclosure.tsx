import { readStored, writeStored } from "./storage";
import { useState, type ReactNode, type SyntheticEvent } from "react";
import { Icon } from "./Icon";

/**
 * A collapsed-by-default inspector tier (Advanced, Reactive) -- a native
 * `<details>`, so keyboard toggling and the expanded/collapsed state are
 * announced for free. Whether it's open is a per-browser convenience,
 * remembered in localStorage by `id` (wrapped: private windows throw).
 */
export function Disclosure({
  id,
  title,
  badge,
  defaultOpen = false,
  children,
}: {
  id: string;
  title: string;
  /** A short status shown after the title, e.g. "2 on". */
  badge?: string;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const suffix = `studio.disclosure.${id}`;
  const [open, setOpen] = useState<boolean>(() => {
    const v = readStored(suffix);
    return v == null ? defaultOpen : v === "1";
  });

  const onToggle = (e: SyntheticEvent<HTMLDetailsElement>) => {
    const next = e.currentTarget.open;
    setOpen(next);
    writeStored(suffix, next ? "1" : "0");
  };

  return (
    <details className="disclosure" open={open} onToggle={onToggle}>
      <summary className="disclosure-summary">
        <Icon name="chevron" />
        <span className="disclosure-title">{title}</span>
        {badge && <span className="disclosure-badge">{badge}</span>}
      </summary>
      <div className="disclosure-body">{children}</div>
    </details>
  );
}

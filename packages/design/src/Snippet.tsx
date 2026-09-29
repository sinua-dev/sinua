import { useState } from "react";
import { Icon } from "./Icon";

/** One platform's tab: the same shape @sinua/snippets exports, so its output passes straight in. */
export interface SnippetTab {
  id: string;
  label: string;
  code: string;
}

export function useCopy(): [copied: boolean, copy: (text: string) => void] {
  const [copied, setCopied] = useState(false);
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {
      // Clipboard API can be unavailable (e.g. insecure context) -- no-op,
      // the code is still selectable/copyable by hand from the <pre>.
    }
  };
  return [copied, copy];
}

/**
 * A standalone code viewer with platform tabs. `CodeDrawer` wraps this same idea as the
 * collapsible workbench drawer. Uncontrolled by default; pass `value` + `onChange` when
 * something else on the page follows the selected tab.
 */
export function Snippet({
  tabs,
  value,
  onChange,
}: {
  tabs: SnippetTab[];
  value?: string;
  onChange?: (id: string) => void;
}) {
  const [ownId, setOwnId] = useState(tabs[0]?.id);
  const activeId = value ?? ownId;
  const setActiveId = (id: string) => {
    setOwnId(id);
    onChange?.(id);
  };
  const current = tabs.find((t) => t.id === activeId) ?? tabs[0];
  const [copied, copy] = useCopy();

  if (!current) return null;

  return (
    <div className="pg-snippet">
      <div className="pg-snippet-tabs" role="tablist" aria-label="Platform">
        {tabs.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={t.id === current.id}
            className={"pg-snippet-tab" + (t.id === current.id ? " pg-snippet-tab-active" : "")}
            onClick={() => setActiveId(t.id)}
          >
            {t.label}
          </button>
        ))}
        <button type="button" className="pg-snippet-copy" onClick={() => copy(current.code)}>
          <Icon name={copied ? "check" : "copy"} />
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <pre className="pg-snippet-code" role="tabpanel">
        <code>{current.code}</code>
      </pre>
    </div>
  );
}

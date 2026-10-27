import { createReactBlockSpec, type DefaultReactSuggestionItem } from "@blocknote/react";
import { insertOrUpdateBlockForSlashMenu } from "@blocknote/core";
import mermaid from "mermaid";
import { useEffect, useId, useState } from "react";
import { FlowArrow } from "@phosphor-icons/react";
import type { ReactElement } from "react";

let mermaidTheme: string | null = null;

function configureMermaid(dark: boolean) {
  const theme = dark ? "dark" : "default";
  if (mermaidTheme === theme) return;
  mermaidTheme = theme;
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme,
  });
}

function useDarkTheme() {
  const [dark, setDark] = useState(
    () => document.documentElement.dataset.theme !== "light",
  );
  useEffect(() => {
    const observer = new MutationObserver(() => {
      setDark(document.documentElement.dataset.theme !== "light");
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);
  return dark;
}

function MermaidPreview({ code }: { code: string }) {
  const dark = useDarkTheme();
  const [svg, setSvg] = useState("");
  const [error, setError] = useState<string | null>(null);
  const renderId = useId().replace(/[^a-zA-Z0-9]/g, "");

  useEffect(() => {
    if (!code.trim()) {
      setSvg("");
      setError(null);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      configureMermaid(dark);
      mermaid
        .render(`kbwrite-mermaid-${renderId}-${Date.now()}`, code)
        .then((result) => {
          if (!cancelled) {
            setSvg(result.svg);
            setError(null);
          }
        })
        .catch((err) => {
          if (!cancelled) {
            setSvg("");
            setError(
              err instanceof Error ? err.message : "Could not render this diagram",
            );
          }
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [code, dark, renderId]);

  if (error) {
    return <div className="mermaid-error">{error}</div>;
  }
  if (!svg) {
    return <p className="muted mermaid-empty">Diagram preview…</p>;
  }
  return (
    <div
      className="mermaid-preview"
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}

export const mermaidBlockSpec = createReactBlockSpec(
  {
    type: "mermaid",
    propSchema: {
      code: { default: "flowchart TD\n    A[Start] --> B[End]" },
    },
    content: "none",
  },
  {
    parse: (element) => {
      const node = element.getAttribute("data-kbwrite-mermaid")
        ? element
        : element.querySelector("[data-kbwrite-mermaid]");
      if (!node) return undefined;
      try {
        return { code: decodeURIComponent(node.getAttribute("data-kbwrite-mermaid") || "") };
      } catch {
        return undefined;
      }
    },
    toExternalHTML: ({ block }) => (
      // BlockNote's markdown exporter turns this into a ```mermaid fence.
      <pre>
        <code className="language-mermaid">{block.props.code}</code>
      </pre>
    ),
    render: ({ block, editor }) => (
      <MermaidBlockView
        code={block.props.code}
        onChange={(next) => editor.updateBlock(block.id, { props: { code: next } })}
      />
    ),
  },
);

function MermaidBlockView({
  code,
  onChange,
}: {
  code: string;
  onChange: (code: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  return (
    <div
      className="mermaid-block"
      contentEditable={false}
      onMouseDown={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
    >
      <div className="mermaid-toolbar">
        <button type="button" onClick={() => setEditing((open) => !open)}>
          {editing ? "Show diagram" : "Edit source"}
        </button>
      </div>
      {editing && (
        <textarea
          className="mermaid-source"
          rows={6}
          value={code}
          spellCheck={false}
          onChange={(event) => onChange(event.target.value)}
          aria-label="Mermaid source"
        />
      )}
      <MermaidPreview code={code} />
    </div>
  );
}

export function mermaidSlashItems(editor: any): DefaultReactSuggestionItem[] {
  return [
    {
      title: "Mermaid diagram",
      subtext: "Render a flowchart, sequence diagram, and more",
      aliases: ["mermaid", "diagram", "flowchart", "chart"],
      group: "Media",
      icon: <FlowArrow size={18} /> as ReactElement,
      onItemClick: () => {
        insertOrUpdateBlockForSlashMenu(editor, { type: "mermaid" });
      },
    },
  ];
}

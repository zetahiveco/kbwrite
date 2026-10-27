import { createReactInlineContentSpec, createReactBlockSpec } from "@blocknote/react";
import { createExtension } from "@blocknote/core";
import { Plugin, PluginKey } from "prosemirror-state";
import type { Node as ProseNode } from "prosemirror-model";
import katex from "katex";
import type { DefaultReactSuggestionItem } from "@blocknote/react";
import { insertOrUpdateBlockForSlashMenu } from "@blocknote/core";
import { Sigma } from "@phosphor-icons/react";
import type { ReactElement } from "react";

function renderMath(latex: string, displayMode: boolean) {
  try {
    return katex.renderToString(latex, {
      displayMode,
      throwOnError: false,
      strict: false,
      output: "html",
    });
  } catch {
    return "";
  }
}

function InlineMath({ latex }: { latex: string }) {
  return <span className="inline-math" dangerouslySetInnerHTML={{ __html: renderMath(latex, false) }} />;
}

export const mathSpec = createReactInlineContentSpec(
  {
    type: "math",
    propSchema: {
      latex: { default: "" },
    },
    content: "none",
  },
  {
    render: ({ inlineContent, updateInlineContent, editor }) => (
      <InlineMathEditor
        latex={inlineContent.props.latex}
        editable={editor.isEditable}
        onChange={(next) => updateInlineContent({ type: "math", props: { latex: next } })}
      />
    ),
    // Serializes back to `$...$` text, so the plugin re-detects it on the next
    // parse without any markdown-level transform.
    toExternalHTML: ({ inlineContent }) => <span>{`$${inlineContent.props.latex}$`}</span>,
  },
);

function InlineMathEditor({
  latex,
  editable,
  onChange,
}: {
  latex: string;
  editable: boolean;
  onChange: (latex: string) => void;
}) {
  if (!editable) {
    return <InlineMath latex={latex} />;
  }
  return (
    <span
      className="inline-math editable"
      contentEditable={false}
      onMouseDown={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
    >
      <InlineMath latex={latex} />
      <input
        className="inline-math-input"
        value={latex}
        spellCheck={false}
        onChange={(event) => onChange(event.target.value)}
        aria-label="Math expression"
      />
    </span>
  );
}

const mathKey = new PluginKey("kbwrite-math");

function mathPlugin() {
  return new Plugin({
    key: mathKey,
    appendTransaction(transactions, _old, state) {
      if (!transactions.some((tr) => tr.docChanged)) return null;
      if (transactions.some((tr) => tr.getMeta("math") || tr.getMeta("history$"))) return null;
      const type = state.schema.nodes.math;
      if (!type) return null;
      const replacements: { from: number; to: number; latex: string }[] = [];
      state.doc.descendants((node, pos) => {
        if (node.type.spec.code) return false;
        if (!node.isText || !node.text) return;
        if (node.marks.some((mark) => mark.type.spec.code)) return;
        collectMath(node, pos, replacements);
      });
      if (replacements.length === 0) return null;
      const tr = state.tr;
      for (const item of replacements.sort((a, b) => b.from - a.from)) {
        tr.replaceWith(item.from, item.to, type.create({ latex: item.latex }));
      }
      tr.setMeta("math", true);
      return tr;
    },
  });
}

function collectMath(
  node: ProseNode,
  pos: number,
  into: { from: number; to: number; latex: string }[],
) {
  const text = node.text ?? "";
  // `$...$`: content cannot start or end with a space, and the opening `$`
  // cannot be escaped or followed by a digit (keeps "$5 and $10" as text).
  const re = /\$([^\s$][^$\n]*[^\s$]|\S)\$/g;
  for (const match of text.matchAll(re)) {
    const index = match.index ?? 0;
    if (text[index - 1] === "\\") continue;
    const after = text[index + match[0].length];
    if (after && /\d/.test(after)) continue;
    into.push({
      from: pos + index,
      to: pos + index + match[0].length,
      latex: match[1] ?? "",
    });
  }
}

export function mathExtension() {
  return createExtension({
    key: "math-convert",
    prosemirrorPlugins: [mathPlugin()],
  });
}

function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export const mathBlockSpec = createReactBlockSpec(
  {
    type: "mathBlock",
    propSchema: {
      latex: { default: "" },
    },
    content: "none",
  },
  {
    parse: (element) => {
      const node = element.getAttribute("data-kbwrite-math")
        ? element
        : element.querySelector("[data-kbwrite-math]");
      if (!node) return undefined;
      try {
        return { latex: decodeURIComponent(node.getAttribute("data-kbwrite-math") || "") };
      } catch {
        return undefined;
      }
    },
    toExternalHTML: ({ block }) => (
      // BlockNote's markdown exporter turns <math> into a `$$ … $$` block.
      <div
        dangerouslySetInnerHTML={{
          __html: `<math><annotation encoding="application/x-tex">${escapeHtml(
            block.props.latex,
          )}</annotation></math>`,
        }}
      />
    ),
    render: ({ block, editor }) => (
      <MathBlockView
        latex={block.props.latex}
        onChange={(next) =>
          editor.updateBlock(block.id, { props: { latex: next } })
        }
      />
    ),
  },
);

function MathBlockView({
  latex,
  onChange,
}: {
  latex: string;
  onChange: (latex: string) => void;
}) {
  return (
    <div
      className="math-block"
      contentEditable={false}
      onMouseDown={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
    >
      <textarea
        className="math-block-source"
        rows={2}
        value={latex}
        spellCheck={false}
        placeholder="e.g. \\int_0^\\infty e^{-x} dx = 1"
        onChange={(event) => onChange(event.target.value)}
        aria-label="Math source"
      />
      {latex.trim() && (
        <div
          className="math-block-preview"
          dangerouslySetInnerHTML={{ __html: renderMath(latex, true) }}
        />
      )}
    </div>
  );
}

export function mathSlashItems(editor: any): DefaultReactSuggestionItem[] {
  return [
    {
      title: "Math",
      subtext: "Display a LaTeX expression",
      aliases: ["math", "latex", "katex", "formula", "equation"],
      group: "Media",
      icon: <Sigma size={18} /> as ReactElement,
      onItemClick: () => {
        insertOrUpdateBlockForSlashMenu(editor, { type: "mathBlock" });
      },
    },
    {
      title: "Inline math",
      subtext: "Insert $...$ math in the text",
      aliases: ["math", "latex", "inline math"],
      group: "Media",
      icon: <Sigma size={18} /> as ReactElement,
      onItemClick: () => {
        editor.insertInlineContent([{ type: "math", props: { latex: "x^2" } }]);
      },
    },
  ];
}

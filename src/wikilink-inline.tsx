import { createExtension, defaultInlineContentSpecs } from "@blocknote/core";
import { createReactInlineContentSpec } from "@blocknote/react";
import { Plugin, PluginKey } from "prosemirror-state";
import type { Node as ProseNode } from "prosemirror-model";
import { formatWikilink, parseWikilinkBody, wikilinkLabel } from "./wikilink";

export const openWikilink: { current: (target: string) => void } = {
  current: () => {},
};

export const wikilinkSpec = createReactInlineContentSpec(
  {
    type: "wikilink",
    propSchema: {
      target: { default: "" },
      alias: { default: "" },
    },
    content: "none",
  },
  {
    render: ({ inlineContent }) => {
      const target = inlineContent.props.target;
      const label = wikilinkLabel(target, inlineContent.props.alias);
      return (
        <a
          className="wikilink"
          href="#"
          title={target}
          onMouseDown={(event) => event.preventDefault()}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            if (target) openWikilink.current(target);
          }}
        >
          {label}
        </a>
      );
    },
    toExternalHTML: ({ inlineContent }) => (
      <span>{formatWikilink(inlineContent.props.target, inlineContent.props.alias)}</span>
    ),
  },
);

export const inlineContentSpecs = {
  ...defaultInlineContentSpecs,
  wikilink: wikilinkSpec,
};

const wikilinkKey = new PluginKey("kbwrite-wikilink");

function wikilinkPlugin() {
  return new Plugin({
    key: wikilinkKey,
    appendTransaction(transactions, _old, state) {
      if (!transactions.some((tr) => tr.docChanged)) return null;
      if (transactions.some((tr) => tr.getMeta("wikilink") || tr.getMeta("history$"))) return null;
      const type = state.schema.nodes.wikilink;
      if (!type) return null;
      const replacements: { from: number; to: number; target: string; alias: string }[] = [];
      state.doc.descendants((node, pos) => {
        if (node.type.spec.code) return false;
        if (!node.isText || !node.text) return;
        if (node.marks.some((mark) => mark.type.spec.code)) return;
        collectWikilinks(node, pos, replacements);
      });
      if (replacements.length === 0) return null;
      const tr = state.tr;
      for (const item of replacements.sort((a, b) => b.from - a.from)) {
        tr.replaceWith(item.from, item.to, type.create({ target: item.target, alias: item.alias }));
      }
      tr.setMeta("wikilink", true);
      return tr;
    },
  });
}

function collectWikilinks(
  node: ProseNode,
  pos: number,
  into: { from: number; to: number; target: string; alias: string }[],
) {
  const text = node.text ?? "";
  const re = /\[\[([^\[\]\n]+)\]\]/g;
  for (const match of text.matchAll(re)) {
    const parts = parseWikilinkBody(match[1] ?? "");
    const index = match.index ?? 0;
    if (!parts) continue;
    into.push({
      from: pos + index,
      to: pos + index + match[0].length,
      target: parts.target,
      alias: parts.alias,
    });
  }
}

export function wikilinkExtension() {
  return createExtension({
    key: "wikilink-convert",
    prosemirrorPlugins: [wikilinkPlugin()],
  });
}

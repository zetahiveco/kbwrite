import { type ReactElement } from "react";
import {
  BlockNoteSchema,
  defaultBlockSpecs,
  insertOrUpdateBlockForSlashMenu,
  type BlockNoteEditor,
} from "@blocknote/core";
import { createReactBlockSpec, type DefaultReactSuggestionItem } from "@blocknote/react";
import { Cards } from "@phosphor-icons/react";
import type { PluginBlock } from "./types";
import { csvBlock, docxBlock, pdfBlock, xlsxBlock } from "./components/media-blocks";
import { inlineContentSpecs as wikilinkInlineContentSpecs } from "./wikilink-inline";
import { codeBlockSpec } from "./code-block";
import { mathBlockSpec, mathSpec } from "./math";
import { mermaidBlockSpec } from "./mermaid";

export function createEditorSchema(blocks: PluginBlock[]): any {
  const blockSpecs: Record<string, any> = {
    ...defaultBlockSpecs,
    codeBlock: codeBlockSpec(),
    mathBlock: mathBlockSpec(),
    mermaid: mermaidBlockSpec(),
    pdf: pdfBlock(),
    csv: csvBlock(),
    docx: docxBlock(),
    xlsx: xlsxBlock(),
  };
  for (const block of blocks) {
    blockSpecs[block.type] = pluginBlockSpec(block)();
  }
  return BlockNoteSchema.create({
    blockSpecs,
    inlineContentSpecs: {
      ...wikilinkInlineContentSpecs,
      math: mathSpec,
    },
  });
}

export function pluginSlashItems(
  editor: BlockNoteEditor<any, any, any>,
  blocks: PluginBlock[],
): DefaultReactSuggestionItem[] {
  return blocks.map((block) => ({
    title: block.title,
    subtext: block.description || block.group,
    aliases: [block.type, block.title.toLowerCase()],
    group: block.group || "Plugins",
    icon: <Cards size={18} /> as ReactElement,
    onItemClick: () => {
      const props: Record<string, string> = {};
      for (const prop of block.props) props[prop.id] = prop.default;
      insertOrUpdateBlockForSlashMenu(editor, { type: block.type as never, props });
    },
  }));
}

function pluginBlockSpec(spec: PluginBlock) {
  const propSchema: Record<string, { default: string }> = {};
  for (const prop of spec.props) propSchema[prop.id] = { default: prop.default };
  return createReactBlockSpec(
    {
      type: spec.type,
      propSchema,
      content: "none",
    },
    {
      parse: (element) => parsePluginBlock(element, spec.type),
      toExternalHTML: ({ block }) => (
        <div
          data-kbwrite-block={spec.type}
          data-props={encodeURIComponent(JSON.stringify(block.props))}
        />
      ),
      render: ({ block, editor }) => (
        <PluginBlockView spec={spec} block={block} editor={editor as BlockNoteEditor<any, any, any>} />
      ),
    },
  );
}

function parsePluginBlock(element: HTMLElement, type: string) {
  const node = element.getAttribute("data-kbwrite-block")
    ? element
    : element.querySelector("[data-kbwrite-block]");
  if (!node || node.getAttribute("data-kbwrite-block") !== type) return undefined;
  try {
    const parsed = JSON.parse(decodeURIComponent(node.getAttribute("data-props") || "")) as Record<string, unknown>;
    const props: Record<string, string> = {};
    for (const [key, value] of Object.entries(parsed)) {
      if (typeof value === "string") props[key] = value;
    }
    return props;
  } catch {
    return undefined;
  }
}

function PluginBlockView({
  spec,
  block,
  editor,
}: {
  spec: PluginBlock;
  block: { id: string; props: Record<string, string> };
  editor: BlockNoteEditor<any, any, any>;
}) {
  return (
    <div
      className="plugin-block"
      onMouseDown={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
    >
      <strong>{spec.title}</strong>
      {spec.props.map((prop) => (
        <label key={prop.id}>
          {prop.label}
          <textarea
            rows={2}
            value={block.props[prop.id] ?? ""}
            onChange={(event) =>
              editor.updateBlock(block.id, {
                props: { ...block.props, [prop.id]: event.target.value },
              })
            }
          />
        </label>
      ))}
    </div>
  );
}

import type { NoteMeta } from "./types";

const WIKILINK = /\[\[([^\[\]\n]+)\]\]/g;

export type WikilinkParts = { target: string; alias: string };

export function parseWikilinkBody(body: string): WikilinkParts | null {
  const trimmed = body.trim();
  if (!trimmed) return null;
  const bar = trimmed.indexOf("|");
  const target = (bar === -1 ? trimmed : trimmed.slice(0, bar)).trim();
  const alias = bar === -1 ? "" : trimmed.slice(bar + 1).trim();
  if (!target) return null;
  return { target, alias };
}

export function formatWikilink(target: string, alias: string) {
  const page = target.trim();
  const name = alias.trim();
  if (!page) return "";
  if (name && name !== page) return `[[${page}|${name}]]`;
  return `[[${page}]]`;
}

export function wikilinkLabel(target: string, alias: string) {
  const name = alias.trim();
  if (name) return name;
  const page = target.split("#")[0].replace(/\.md$/i, "").trim();
  return page || target.trim();
}

export function findNotePath(notes: NoteMeta[], rawTarget: string) {
  const page = rawTarget.split("#")[0].trim();
  if (!page) return null;
  const key = page.replace(/\\/g, "/").replace(/\.md$/i, "").toLowerCase();
  let found: string | null = null;
  for (const note of notes) {
    const rel = note.rel.replace(/\\/g, "/");
    const relKey = rel.replace(/\.md$/i, "").toLowerCase();
    if (
      note.title.trim().toLowerCase() === key ||
      relKey === key ||
      rel.toLowerCase() === page.toLowerCase()
    ) {
      found = note.path;
    }
  }
  return found;
}

type TextInline = {
  type: "text";
  text: string;
  styles?: Record<string, unknown>;
};

type InlineItem = {
  type: string;
  text?: string;
  styles?: Record<string, unknown>;
  props?: Record<string, unknown>;
  content?: unknown;
  href?: string;
};

export function expandWikilinks<T>(blocks: T[]): T[] {
  return blocks.map((block) => expandBlock(block)) as T[];
}

function expandBlock(block: unknown): unknown {
  if (!block || typeof block !== "object") return block;
  const source = block as Record<string, unknown>;
  const next: Record<string, unknown> = { ...source };
  if (source.type !== "codeBlock") {
    if (isTableContent(source.content)) next.content = expandTable(source.content);
    else if (Array.isArray(source.content)) next.content = expandInline(source.content);
    else if (typeof source.content === "string") {
      next.content = expandInline([{ type: "text", text: source.content, styles: {} }]);
    }
  }
  if (Array.isArray(source.children)) next.children = source.children.map((child) => expandBlock(child));
  return next;
}

function isTableContent(content: unknown): content is { type: "tableContent"; rows: { cells: unknown[] }[] } {
  return !!content && typeof content === "object" && (content as { type?: string }).type === "tableContent";
}

function expandTable(table: { type: "tableContent"; rows: { cells: unknown[] }[] }) {
  return {
    ...table,
    rows: table.rows.map((row) => ({
      ...row,
      cells: row.cells.map((cell) => {
        if (Array.isArray(cell)) return expandInline(cell);
        if (cell && typeof cell === "object" && Array.isArray((cell as { content?: unknown }).content)) {
          const item = cell as { content: unknown[] };
          return { ...item, content: expandInline(item.content) };
        }
        return cell;
      }),
    })),
  };
}

function expandInline(content: unknown[]) {
  const out: unknown[] = [];
  for (const item of content) {
    if (!isInline(item)) {
      out.push(item);
      continue;
    }
    if (item.type === "link" && Array.isArray(item.content)) {
      out.push({ ...item, content: expandInline(item.content) });
      continue;
    }
    if (item.type !== "text" || typeof item.text !== "string" || item.styles?.code) {
      out.push(item);
      continue;
    }
    out.push(...splitText(item as TextInline));
  }
  return out;
}

function isInline(item: unknown): item is InlineItem {
  return !!item && typeof item === "object" && typeof (item as { type?: unknown }).type === "string";
}

function splitText(item: TextInline) {
  const out: unknown[] = [];
  const re = new RegExp(WIKILINK.source, "g");
  let last = 0;
  let matched = false;
  for (const match of item.text.matchAll(re)) {
    const parts = parseWikilinkBody(match[1] ?? "");
    const index = match.index ?? 0;
    if (!parts) continue;
    matched = true;
    const before = item.text.slice(last, index);
    if (before) out.push({ ...item, text: before });
    out.push({ type: "wikilink", props: { target: parts.target, alias: parts.alias } });
    last = index + match[0].length;
  }
  if (!matched) return [item];
  const after = item.text.slice(last);
  if (after) out.push({ ...item, text: after });
  return out;
}

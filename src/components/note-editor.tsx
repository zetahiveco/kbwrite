import { useEffect, useMemo, useRef, useState, type ReactElement } from "react";
import {
  FilePanelExtension,
  FormattingToolbarExtension,
  insertOrUpdateBlockForSlashMenu,
  type BlockNoteEditor,
} from "@blocknote/core";
import { filterSuggestionItems } from "@blocknote/core/extensions";
import { BlockNoteView } from "@blocknote/mantine";
import {
  SuggestionMenuController,
  getDefaultReactSlashMenuItems,
  useCreateBlockNote,
  useEditorChange,
  type DefaultReactSuggestionItem,
} from "@blocknote/react";
import { FileCsv, FileDoc, FilePdf, FileXls, LinkSimple } from "@phosphor-icons/react";
import { saveMedia } from "../api";
import { fileToBase64, mediaKind, restoreMediaMarkdown, displayUrl } from "../media";
import type { NoteMeta, PluginBlock } from "../types";
import { expandWikilinks, findNotePath } from "../wikilink";
import { openWikilink, wikilinkExtension } from "../wikilink-inline";
import { createEditorSchema, pluginSlashItems } from "../plugin-blocks";
import { Dialog } from "./dialog";

const HIDDEN_SLASH = new Set(["File"]);

// BlockNote keys each section label by its group name. The same name twice
// (built-in media, then PDF/CSV/DOCX/XLSX) leaves the first labels stuck on
// screen after the query filters them out.
function organizeSlashItems(items: DefaultReactSuggestionItem[]) {
  const order: string[] = [];
  const grouped = new Map<string, DefaultReactSuggestionItem[]>();
  for (const item of items) {
    const group = item.group ?? "";
    let bucket = grouped.get(group);
    if (!bucket) {
      bucket = [];
      grouped.set(group, bucket);
      order.push(group);
    }
    bucket.push(item);
  }
  return order.flatMap((group) => grouped.get(group)!);
}

export function NoteEditor({
  notePath,
  initialMarkdown,
  dark,
  fullWidth,
  notes,
  pluginBlocks,
  onMarkdown,
  onEditor,
  onError,
  onOpen,
}: {
  notePath: string;
  initialMarkdown: string;
  dark: boolean;
  fullWidth: boolean;
  notes: NoteMeta[];
  pluginBlocks: PluginBlock[];
  onMarkdown: (markdown: string) => void;
  onEditor: (editor: BlockNoteEditor<any, any, any> | null) => void;
  onError: (message: string) => void;
  onOpen: (path: string) => void;
}) {
  const schema = useMemo(() => createEditorSchema(pluginBlocks), [pluginBlocks]);
  const editor = useCreateBlockNote({
    schema,
    extensions: [wikilinkExtension()],
    uploadFile: async (file) => {
      if (!mediaKind(file.name)) {
        throw new Error("Use an image, PDF, CSV, DOCX, XLSX, video, or audio file.");
      }
      if (file.size > 80 * 1024 * 1024) {
        throw new Error("Files must be 80 MB or smaller.");
      }
      const url = await saveMedia(notePath, file.name, await fileToBase64(file));
      return { props: { url, name: file.name } };
    },
    resolveFileUrl: async (url) => displayUrl(url),
  });
  const initialRef = useRef(initialMarkdown);
  const hydrating = useRef(true);
  const onMarkdownRef = useRef(onMarkdown);
  const onErrorRef = useRef(onError);
  const onOpenRef = useRef(onOpen);
  const notesRef = useRef(notes);
  onMarkdownRef.current = onMarkdown;
  onErrorRef.current = onError;
  onOpenRef.current = onOpen;
  notesRef.current = notes;
  const [linkOpen, setLinkOpen] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    onEditor(editor);
    return () => onEditor(null);
  }, [editor, onEditor]);

  useEffect(() => {
    openWikilink.current = (target) => {
      const path = findNotePath(notesRef.current, target);
      if (!path) {
        const name = target.split("#")[0].trim() || target;
        onErrorRef.current(`No note named “${name}”`);
        return;
      }
      onOpenRef.current(path);
    };
    return () => {
      openWikilink.current = () => {};
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const blocks = await editor.tryParseMarkdownToBlocks(
          restoreMediaMarkdown(initialRef.current || ""),
        );
        if (cancelled) return;
        const linked = expandWikilinks(blocks);
        if (linked.length > 0) {
          editor.replaceBlocks(editor.document, linked);
        }
      } catch (error) {
        onErrorRef.current(
          error instanceof Error ? error.message : "Could not open this note",
        );
      } finally {
        if (!cancelled) {
          hydrating.current = false;
          setReady(true);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [editor]);

  useEditorChange(async (instance) => {
    if (hydrating.current) return;
    const markdown = await instance.blocksToMarkdownLossy(instance.document);
    onMarkdownRef.current(markdown);
  }, editor);

  return (
    <div className={`editor-frame${fullWidth ? " full-width" : ""}${ready ? "" : " editor-pending"}`}>
      <BlockNoteView editor={editor} theme={dark ? "dark" : "light"} slashMenu={false}>
        <SuggestionMenuController
          triggerCharacter="/"
          getItems={async (query) =>
            filterSuggestionItems(
              organizeSlashItems([
                ...getDefaultReactSlashMenuItems(editor).filter(
                  (item) => !HIDDEN_SLASH.has(item.title),
                ),
                linkItem(() => setLinkOpen(true)),
                ...documentItems(editor),
                ...pluginSlashItems(editor, pluginBlocks),
              ]),
              query,
            )
          }
        />
      </BlockNoteView>
      {linkOpen && (
        <LinkPicker
          notes={notes}
          onClose={() => setLinkOpen(false)}
          onPick={(title) => {
            const target = title.replace(/[\[\]]/g, "");
            editor.insertInlineContent([
              { type: "wikilink", props: { target, alias: "" } },
            ] as any);
            setLinkOpen(false);
          }}
        />
      )}
    </div>
  );
}

function documentItems(editor: BlockNoteEditor<any, any, any>): DefaultReactSuggestionItem[] {
  return [
    documentItem(editor, "pdf", "PDF", "Embed a PDF", ["pdf", "document"], <FilePdf size={18} />),
    documentItem(editor, "csv", "CSV", "Embed a spreadsheet of rows", ["csv", "table", "sheet"], <FileCsv size={18} />),
    documentItem(editor, "docx", "DOCX", "Embed a Word document", ["docx", "word", "doc"], <FileDoc size={18} />),
    documentItem(editor, "xlsx", "XLSX", "Embed an Excel workbook", ["xlsx", "excel", "sheet"], <FileXls size={18} />),
  ];
}

function documentItem(
  editor: BlockNoteEditor<any, any, any>,
  type: "pdf" | "csv" | "docx" | "xlsx",
  title: string,
  subtext: string,
  aliases: string[],
  icon: ReactElement,
): DefaultReactSuggestionItem {
  return {
    title,
    subtext,
    aliases,
    group: "Media",
    icon,
    onItemClick: () => {
      const inserted = insertOrUpdateBlockForSlashMenu(editor, { type });
      editor.getExtension(FilePanelExtension)?.showMenu(inserted.id);
      editor.getExtension(FormattingToolbarExtension)?.store.setState(false);
    },
  };
}

function linkItem(onPick: () => void): DefaultReactSuggestionItem {
  return {
    title: "Link to note",
    subtext: "Insert a [[wikilink]]",
    aliases: ["link", "wiki", "note", "page", "reference"],
    icon: <LinkSimple size={18} />,
    onItemClick: onPick,
  };
}

function LinkPicker({
  notes,
  onClose,
  onPick,
}: {
  notes: NoteMeta[];
  onClose: () => void;
  onPick: (title: string) => void;
}) {
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  const matches = notes
    .filter((note) => !q || note.title.toLowerCase().includes(q) || note.rel.toLowerCase().includes(q))
    .slice(0, 40);

  return (
    <Dialog title="Link to note" onClose={onClose}>
      <input
        className="field"
        autoFocus
        value={query}
        placeholder="Search notes"
        onChange={(event) => setQuery(event.target.value)}
      />
      <div className="link-list">
        {matches.length === 0 && <p className="muted">No matching notes.</p>}
        {matches.map((note) => (
          <button key={note.path} type="button" className="link-row" onClick={() => onPick(note.title)}>
            <span>{note.title}</span>
            <small>{note.rel.replace(/\.md$/i, "")}</small>
          </button>
        ))}
      </div>
    </Dialog>
  );
}

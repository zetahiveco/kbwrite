import { useEffect, useRef, useState, type ReactElement } from "react";
import {
  BlockNoteSchema,
  FilePanelExtension,
  FormattingToolbarExtension,
  defaultBlockSpecs,
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
import type { NoteMeta } from "../types";
import { Dialog } from "./Dialog";
import { csvBlock, docxBlock, pdfBlock, xlsxBlock } from "./mediaBlocks";

const HIDDEN_SLASH = new Set(["File"]);

const schema = BlockNoteSchema.create({
  blockSpecs: {
    ...defaultBlockSpecs,
    pdf: pdfBlock(),
    csv: csvBlock(),
    docx: docxBlock(),
    xlsx: xlsxBlock(),
  },
});

export function NoteEditor({
  notePath,
  initialMarkdown,
  dark,
  fullWidth,
  notes,
  onMarkdown,
  onEditor,
  onError,
}: {
  notePath: string;
  initialMarkdown: string;
  dark: boolean;
  fullWidth: boolean;
  notes: NoteMeta[];
  onMarkdown: (markdown: string) => void;
  onEditor: (editor: BlockNoteEditor<any, any, any> | null) => void;
  onError: (message: string) => void;
}) {
  const editor = useCreateBlockNote({
    schema,
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
  onMarkdownRef.current = onMarkdown;
  onErrorRef.current = onError;
  const [linkOpen, setLinkOpen] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    onEditor(editor);
    return () => onEditor(null);
  }, [editor, onEditor]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const blocks = await editor.tryParseMarkdownToBlocks(
          restoreMediaMarkdown(initialRef.current || ""),
        );
        if (cancelled) return;
        if (blocks.length > 0) {
          editor.replaceBlocks(editor.document, blocks);
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
              [
                ...getDefaultReactSlashMenuItems(editor).filter(
                  (item) => !HIDDEN_SLASH.has(item.title),
                ),
                linkItem(() => setLinkOpen(true)),
                ...documentItems(editor),
              ],
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
            editor.insertInlineContent(`[[${title.replace(/[\[\]]/g, "")}]]`);
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
    group: "KbWrite",
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

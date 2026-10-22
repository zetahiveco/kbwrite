import { useEffect, useRef, useState } from "react";
import { filterSuggestionItems } from "@blocknote/core/extensions";
import { BlockNoteView } from "@blocknote/mantine";
import {
  SuggestionMenuController,
  getDefaultReactSlashMenuItems,
  useCreateBlockNote,
  useEditorChange,
  type DefaultReactSuggestionItem,
} from "@blocknote/react";
import type { BlockNoteEditor } from "@blocknote/core";
import { LinkSimple } from "@phosphor-icons/react";
import type { NoteMeta } from "../types";
import { Dialog } from "./Dialog";

const HIDDEN_SLASH = new Set(["Image", "Video", "Audio", "File"]);

export function NoteEditor({
  initialMarkdown,
  dark,
  fullWidth,
  notes,
  onMarkdown,
  onEditor,
  onError,
}: {
  initialMarkdown: string;
  dark: boolean;
  fullWidth: boolean;
  notes: NoteMeta[];
  onMarkdown: (markdown: string) => void;
  onEditor: (editor: BlockNoteEditor | null) => void;
  onError: (message: string) => void;
}) {
  const editor = useCreateBlockNote();
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
        const blocks = await editor.tryParseMarkdownToBlocks(initialRef.current || "");
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

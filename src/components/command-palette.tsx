import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowRight, Bookmark, MagnifyingGlass } from "@phosphor-icons/react";
import { errorMessage, searchVault } from "../api";
import type { NoteMeta, SearchHit } from "../types";

export type PaletteMode = "commands" | "files" | "search";

export type PaletteCommand = {
  id: string;
  label: string;
  hint?: string;
  shortcut?: string;
  run: () => void;
};

type Row =
  | { kind: "command"; command: PaletteCommand }
  | { kind: "note"; note: NoteMeta }
  | { kind: "hit"; hit: SearchHit };

export function CommandPalette({
  mode,
  commands,
  notes,
  bookmarks,
  currentPath,
  onClose,
  onOpenNote,
  onModeChange,
}: {
  mode: PaletteMode;
  commands: PaletteCommand[];
  notes: NoteMeta[];
  bookmarks: string[];
  currentPath: string | null;
  onClose: () => void;
  onOpenNote: (path: string) => void;
  onModeChange: (mode: PaletteMode) => void;
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, [mode]);

  // Reset transient state when the mode or query changes.
  useEffect(() => {
    setSelected(0);
    setHits(null);
    setSearchError(null);
  }, [mode, query]);

  // Global search runs against the vault with a small debounce.
  useEffect(() => {
    if (mode !== "search" || !query.trim()) return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      searchVault(query)
        .then((result) => {
          if (!cancelled) {
            setHits(result);
            setSearchError(null);
          }
        })
        .catch((error) => {
          if (!cancelled) setSearchError(errorMessage(error));
        });
    }, 200);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [mode, query]);

  const bookmarked = useMemo(() => new Set(bookmarks), [bookmarks]);

  const rows = useMemo<Row[]>(() => {
    const q = query.trim().toLowerCase();
    if (mode === "search") {
      return (hits ?? []).map((hit) => ({ kind: "hit", hit }) as Row);
    }
    const noteRows = notes
      .filter(
        (note) =>
          !q ||
          note.title.toLowerCase().includes(q) ||
          note.rel.toLowerCase().includes(q),
      )
      .sort((a, b) => rank(b, q) - rank(a, q))
      .slice(0, mode === "files" ? 30 : 8)
      .map((note) => ({ kind: "note", note }) as Row);
    if (mode === "files") return noteRows;
    const commandRows = commands
      .filter((command) => !q || command.label.toLowerCase().includes(q))
      .map((command) => ({ kind: "command", command }) as Row);
    return [...commandRows, ...noteRows];
  }, [mode, query, commands, notes, hits]);

  useEffect(() => {
    const row = listRef.current?.querySelector(".palette-row.active");
    row?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  function activate(row: Row | undefined) {
    if (!row) return;
    if (row.kind === "command") {
      onClose();
      row.command.run();
    } else if (row.kind === "note") {
      onClose();
      onOpenNote(row.note.path);
    } else {
      onClose();
      onOpenNote(row.hit.path);
    }
  }

  function onKeyDown(event: React.KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected((current) => Math.min(current + 1, rows.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((current) => Math.max(current - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      activate(rows[selected]);
    }
  }

  return (
    <div className="palette-overlay" onMouseDown={onClose}>
      <div
        className="palette"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="palette-input-row">
          <MagnifyingGlass size={15} />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={onKeyDown}
            placeholder={
              mode === "search"
                ? "Search all notes…"
                : mode === "files"
                  ? "Go to file…"
                  : "Type a command or note name…"
            }
            aria-label={mode === "search" ? "Global search" : "Command palette"}
            role="combobox"
            aria-expanded
            aria-controls="palette-list"
          />
        </div>
        <div className="palette-list" ref={listRef} id="palette-list" role="listbox">
          {mode === "search" && searchError && <p className="muted palette-note">{searchError}</p>}
          {rows.length === 0 && (
            <p className="muted palette-note">
              {mode === "search" && query.trim()
                ? "No matches in this vault."
                : "Nothing matches."}
            </p>
          )}
          {rows.map((row, index) => (
            <RowView
              key={rowKey(row, index)}
              row={row}
              active={index === selected}
              bookmarked={
                (row.kind === "note" && bookmarked.has(row.note.path)) ||
                (row.kind === "hit" && bookmarked.has(row.hit.path))
              }
              current={row.kind === "note" && row.note.path === currentPath}
              onMouseEnter={() => setSelected(index)}
              onRun={() => activate(row)}
            />
          ))}
        </div>
        <footer className="palette-foot">
          {mode !== "commands" && (
            <button type="button" onClick={() => onModeChange("commands")}>
              Commands
            </button>
          )}
          {mode !== "files" && (
            <button type="button" onClick={() => onModeChange("files")}>
              Files
            </button>
          )}
          {mode !== "search" && (
            <button type="button" onClick={() => onModeChange("search")}>
              Full-text search
            </button>
          )}
          <span className="muted">↑↓ to move · Enter to run · Esc to close</span>
        </footer>
      </div>
    </div>
  );
}

function RowView({
  row,
  active,
  bookmarked,
  current,
  onRun,
  onMouseEnter,
}: {
  row: Row;
  active: boolean;
  bookmarked: boolean;
  current: boolean;
  onRun: () => void;
  onMouseEnter: () => void;
}) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={active}
      className={`palette-row${active ? " active" : ""}${current ? " current" : ""}`}
      onMouseEnter={onMouseEnter}
      onClick={onRun}
    >
      {row.kind === "command" ? (
        <>
          <span className="palette-label">
            {row.command.label}
            {row.command.shortcut && <kbd>{row.command.shortcut}</kbd>}
          </span>
          {row.command.hint && <small className="muted">{row.command.hint}</small>}
        </>
      ) : row.kind === "note" ? (
        <>
          <span className="palette-label">
            {bookmarked && <Bookmark size={12} weight="fill" />}
            {row.note.title}
          </span>
          <small className="muted">{row.note.rel.replace(/\.md$/i, "")}</small>
        </>
      ) : (
        <>
          <span className="palette-label">
            <ArrowRight size={12} className="palette-glyph" />
            Line {row.hit.line}: {row.hit.snippet}
          </span>
          <small className="muted">{row.hit.rel.replace(/\.md$/i, "")}</small>
        </>
      )}
    </button>
  );
}

function rowKey(row: Row, index: number) {
  if (row.kind === "command") return row.command.id;
  if (row.kind === "note") return row.note.path;
  return `${row.hit.path}:${row.hit.line}:${index}`;
}

function rank(note: NoteMeta, query: string) {
  if (!query) return 0;
  const title = note.title.toLowerCase();
  const at = title.indexOf(query);
  if (at === 0) return 2;
  if (at > 0) return 1;
  return 0;
}

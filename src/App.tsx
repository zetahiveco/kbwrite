import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { BlockNoteEditor } from "@blocknote/core";
import {
  Code,
  FileText,
  FloppyDisk,
  Graph,
  Moon,
  NoteBlank,
  PencilSimple,
  Sun,
  Trash,
} from "@phosphor-icons/react";
import {
  createFile,
  createFolder,
  deletePath,
  errorMessage,
  getSnapshot,
  graph,
  openPrivacySettings,
  readNote,
  reloadVault,
  renamePath,
  saveSettings,
  writeNote,
} from "./api";
import { Dialog } from "./components/Dialog";
import { GraphView } from "./components/GraphView";
import { NoteEditor } from "./components/NoteEditor";
import { SettingsDialog } from "./components/SettingsDialog";
import { Sidebar } from "./components/Sidebar";
import type { GraphData, Settings, Snapshot, ViewMode } from "./types";

type Toast = { id: number; message: string };
type PendingDelete = { path: string; name: string; isDir: boolean };

export function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);
  const [currentPath, setCurrentPath] = useState<string | null>(null);
  const [markdown, setMarkdown] = useState("");
  const [dirty, setDirty] = useState(false);
  const [view, setView] = useState<ViewMode>("editor");
  const [editorKey, setEditorKey] = useState(0);
  const [showSettings, setShowSettings] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<PendingDelete | null>(null);
  const [graphData, setGraphData] = useState<GraphData | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [sidebarWidth, setSidebarWidth] = useState(readSidebarWidth);
  const [fullWidth, setFullWidth] = useState(readFullWidth);

  const markdownRef = useRef("");
  const dirtyRef = useRef(false);
  const currentPathRef = useRef<string | null>(null);
  const viewRef = useRef<ViewMode>("editor");
  const editorRef = useRef<BlockNoteEditor | null>(null);
  const rawRef = useRef<HTMLTextAreaElement>(null);
  const saveQueue = useRef(Promise.resolve());
  const saveTimer = useRef<number | null>(null);
  const toastId = useRef(0);
  const actions = useRef({
    save: async () => {},
    newFile: () => {},
    toggleSettings: () => {},
    deleteCurrent: () => {},
  });

  const toast = useCallback((message: string) => {
    const id = ++toastId.current;
    setToasts((current) => [...current, { id, message }]);
    window.setTimeout(() => {
      setToasts((current) => current.filter((item) => item.id !== id));
    }, 4000);
  }, []);

  const onEditor = useCallback((editor: BlockNoteEditor | null) => {
    editorRef.current = editor;
  }, []);

  const saveCurrent = useCallback(async () => {
    if (saveTimer.current) {
      window.clearTimeout(saveTimer.current);
      saveTimer.current = null;
    }
    const path = currentPathRef.current;
    if (!path) return;
    const run = saveQueue.current.then(async () => {
      let content = markdownRef.current;
      if (viewRef.current === "editor" && editorRef.current) {
        content = await editorRef.current.blocksToMarkdownLossy(editorRef.current.document);
        markdownRef.current = content;
        setMarkdown(content);
      }
      await writeNote(path, content);
      dirtyRef.current = false;
      setDirty(false);
    });
    saveQueue.current = run.then(
      () => undefined,
      () => undefined,
    );
    try {
      await run;
    } catch (error) {
      dirtyRef.current = true;
      setDirty(true);
      toast(errorMessage(error));
      throw error;
    }
  }, [toast]);

  const markDirty = useCallback((next: string) => {
    markdownRef.current = next;
    setMarkdown(next);
    if (!dirtyRef.current) {
      dirtyRef.current = true;
      setDirty(true);
    }
    if (saveTimer.current) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      void saveCurrent();
    }, 900);
  }, [saveCurrent]);

  const openNote = useCallback(
    async (path: string, save = true) => {
      if (path === currentPathRef.current && viewRef.current === "editor") return;
      if (save) {
        try {
          await saveCurrent();
        } catch {
          return;
        }
      }
      const content = await readNote(path);
      currentPathRef.current = path;
      markdownRef.current = content;
      dirtyRef.current = false;
      viewRef.current = "editor";
      setCurrentPath(path);
      setMarkdown(content);
      setDirty(false);
      setView("editor");
      setEditorKey((key) => key + 1);
    },
    [saveCurrent],
  );

  const showView = useCallback(
    async (next: ViewMode) => {
      if (viewRef.current === "editor" && editorRef.current) {
        const content = await editorRef.current.blocksToMarkdownLossy(editorRef.current.document);
        markdownRef.current = content;
        setMarkdown(content);
      }
      if (next === "editor" && viewRef.current !== "editor") {
        setEditorKey((key) => key + 1);
      }
      viewRef.current = next;
      setView(next);
    },
    [],
  );

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const loaded = await getSnapshot();
        if (cancelled) return;
        setSnap(loaded);
        setSettings(loaded.settings);
        const first = loaded.notes[0];
        if (first) {
          const content = await readNote(first.path);
          if (cancelled) return;
          currentPathRef.current = first.path;
          markdownRef.current = content;
          setCurrentPath(first.path);
          setMarkdown(content);
        }
      } catch (error) {
        if (!cancelled) setBootError(errorMessage(error));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = settings?.darkMode === false ? "light" : "dark";
  }, [settings?.darkMode]);

  const reloadExternal = useCallback(async () => {
    try {
      const loaded = await getSnapshot();
      setSnap(loaded);
      setSettings(loaded.settings);
      const path = currentPathRef.current;
      if (!path) return;
      const still = loaded.notes.some((note) => note.path === path);
      if (!still) {
        const next = loaded.notes[0];
        if (!next) {
          currentPathRef.current = null;
          markdownRef.current = "";
          dirtyRef.current = false;
          setCurrentPath(null);
          setMarkdown("");
          setDirty(false);
          return;
        }
        const content = await readNote(next.path);
        currentPathRef.current = next.path;
        markdownRef.current = content;
        dirtyRef.current = false;
        setCurrentPath(next.path);
        setMarkdown(content);
        setDirty(false);
        setEditorKey((key) => key + 1);
        return;
      }
      if (dirtyRef.current) return;
      const content = await readNote(path);
      if (content !== markdownRef.current) {
        markdownRef.current = content;
        setMarkdown(content);
        setEditorKey((key) => key + 1);
      }
    } catch (error) {
      toast(errorMessage(error));
    }
  }, [toast]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen("vault-changed", () => {
      void reloadExternal();
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [reloadExternal]);

  useEffect(() => {
    const note = snap?.notes.find((item) => item.path === currentPath);
    const title = note ? `KbWrite — ${note.title}` : "KbWrite";
    void getCurrentWindow().setTitle(title).catch(() => undefined);
  }, [snap, currentPath]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onCloseRequested(async (event) => {
        event.preventDefault();
        try {
          await saveCurrent();
        } catch {
          /* saveCurrent already toasted */
        }
        await getCurrentWindow().destroy();
      })
      .then((stop) => {
        unlisten = stop;
      })
      .catch(() => undefined);
    return () => unlisten?.();
  }, [saveCurrent]);

  useEffect(() => {
    if (view !== "graph") return;
    let cancelled = false;
    void (async () => {
      try {
        await saveCurrent();
        const data = await graph();
        if (!cancelled) setGraphData(data);
      } catch (error) {
        if (!cancelled) toast(errorMessage(error));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [view, saveCurrent, toast]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const meta = event.metaKey || event.ctrlKey;
      if (!meta) return;
      const key = event.key.toLowerCase();
      if (key === "s") {
        event.preventDefault();
        void actions.current.save();
      } else if (key === "n") {
        event.preventDefault();
        actions.current.newFile();
      } else if (key === ",") {
        event.preventDefault();
        actions.current.toggleSettings();
      } else if (key === "backspace") {
        const target = event.target as HTMLElement | null;
        if (target?.closest("input, textarea, [contenteditable='true']")) return;
        event.preventDefault();
        actions.current.deleteCurrent();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const currentNote = snap?.notes.find((note) => note.path === currentPath) ?? null;

  async function toggleTheme() {
    if (!settings) return;
    const next = { ...settings, darkMode: !settings.darkMode };
    setSettings(next);
    try {
      const updated = await saveSettings(next);
      setSnap(updated);
      setSettings(updated.settings);
    } catch (error) {
      toast(errorMessage(error));
    }
  }

  async function applySettings(next: Settings) {
    const updated = await saveSettings(next);
    setSnap(updated);
    setSettings(updated.settings);
    setShowSettings(false);
    toast("Settings saved");
    const stillThere = updated.notes.some((note) => note.path === currentPathRef.current);
    if (!stillThere && updated.notes[0]) {
      await openNote(updated.notes[0].path, false);
    } else if (!stillThere) {
      currentPathRef.current = null;
      setCurrentPath(null);
      setMarkdown("");
    }
  }

  async function retryVault() {
    if (!settings) return;
    try {
      const updated = await reloadVault(settings.vaultPath);
      setSnap(updated);
      setSettings(updated.settings);
      if (updated.notes[0]) await openNote(updated.notes[0].path, false);
    } catch (error) {
      toast(errorMessage(error));
    }
  }

  async function removePath(target: PendingDelete) {
    const affected =
      currentPathRef.current !== null && pathInside(currentPathRef.current, target.path);
    try {
      if (!affected) await saveCurrent();
      const updated = await deletePath(target.path);
      setSnap(updated);
      setSettings(updated.settings);
      setPendingDelete(null);
      toast("Deleted");
      if (affected) {
        if (updated.notes[0]) await openNote(updated.notes[0].path, false);
        else {
          currentPathRef.current = null;
          setCurrentPath(null);
          setMarkdown("");
        }
      }
    } catch (error) {
      toast(errorMessage(error));
    }
  }

  actions.current = {
    save: async () => {
      try {
        await saveCurrent();
        toast("Note saved");
      } catch {
        /* toasted in saveCurrent */
      }
    },
    newFile: () => {
      if (!settings || !snap) return;
      const dir = settings.vaultPath;
      const name = nextUntitled(snap.tree);
      void (async () => {
        try {
          await saveCurrent();
          const created = await createFile(dir, name);
          setSnap(created.snapshot);
          setSettings(created.snapshot.settings);
          await openNote(created.path, false);
        } catch (error) {
          toast(errorMessage(error));
        }
      })();
    },
    toggleSettings: () => setShowSettings((open) => !open),
    deleteCurrent: () => {
      if (!currentNote) return;
      setPendingDelete({ path: currentNote.path, name: currentNote.title, isDir: false });
    },
  };

  function chooseWidth(full: boolean) {
    localStorage.setItem("kbwrite.editorWidth", full ? "full" : "reduced");
    setFullWidth(full);
  }

  function startResize(event: ReactPointerEvent<HTMLDivElement>) {
    const startX = event.clientX;
    const startWidth = sidebarWidth;
    const onMove = (move: globalThis.PointerEvent) => {
      const next = Math.min(480, Math.max(180, startWidth + move.clientX - startX));
      setSidebarWidth(next);
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      setSidebarWidth((width) => {
        localStorage.setItem("kbwrite.sidebar", String(width));
        return width;
      });
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  if (bootError) {
    return (
      <div className="boot">
        <p>{bootError}</p>
        <button type="button" onClick={() => window.location.reload()}>
          Try again
        </button>
      </div>
    );
  }

  if (!snap || !settings) {
    return <div className="boot">Opening vault…</div>;
  }

  return (
    <div className="shell">
      <div className="sidebar-slot" style={{ width: sidebarWidth }}>
        <Sidebar
          key={settings.vaultPath}
          vaultPath={settings.vaultPath}
          tree={snap.tree}
          currentPath={currentPath}
          vaultError={snap.vaultError}
          walkErrors={snap.walkErrors}
          permissionDenied={snap.permissionDenied}
          onOpen={(path) => void openNote(path).catch((error) => toast(errorMessage(error)))}
          onCreateFile={async (dir, name) => {
            await saveCurrent();
            const created = await createFile(dir, name);
            setSnap(created.snapshot);
            setSettings(created.snapshot.settings);
            await openNote(created.path, false);
          }}
          onCreateFolder={async (dir, name) => {
            const created = await createFolder(dir, name);
            setSnap(created.snapshot);
            setSettings(created.snapshot.settings);
          }}
          onRename={async (from, name) => {
            const renamed = await renamePath(from, name);
            setSnap(renamed.snapshot);
            setSettings(renamed.snapshot.settings);
            if (currentPathRef.current === from) {
              currentPathRef.current = renamed.path;
              setCurrentPath(renamed.path);
            }
          }}
          onDelete={setPendingDelete}
          onOpenSettings={() => setShowSettings(true)}
          onRetry={() => void retryVault()}
          onPrivacy={() => void openPrivacySettings().catch((error) => toast(errorMessage(error)))}
        />
      </div>
      <div className="resizer" onPointerDown={startResize} />
      <main className="main">
        <header className="toolbar">
          <div className="toolbar-title">
            <FileText size={16} />
            <strong>{currentNote?.title ?? "No note"}</strong>
            {dirty && <span className="unsaved">Unsaved</span>}
          </div>
          <div className="toolbar-actions">
            {view === "editor" && <span className="hint">Type / for blocks</span>}
            {(view === "editor" || view === "raw") && (
              <div className="segmented" role="group" aria-label="Editor width">
                <button
                  type="button"
                  className={fullWidth ? "" : "active"}
                  onClick={() => chooseWidth(false)}
                >
                  Reduced
                </button>
                <button
                  type="button"
                  className={fullWidth ? "active" : ""}
                  onClick={() => chooseWidth(true)}
                >
                  Full
                </button>
              </div>
            )}
            <div className="segmented" role="tablist">
              <button
                type="button"
                className={view === "editor" ? "active" : ""}
                onClick={() => void showView("editor")}
              >
                <PencilSimple size={14} /> Editor
              </button>
              <button
                type="button"
                className={view === "raw" ? "active" : ""}
                onClick={() => void showView("raw")}
              >
                <Code size={14} /> Raw
              </button>
              <button
                type="button"
                className={view === "graph" ? "active" : ""}
                onClick={() => void showView("graph")}
              >
                <Graph size={14} /> Graph
              </button>
            </div>
            <button
              type="button"
              className="icon-btn"
              title={settings.darkMode ? "Switch to light mode" : "Switch to dark mode"}
              onClick={() => void toggleTheme()}
            >
              {settings.darkMode ? <Sun size={16} /> : <Moon size={16} />}
            </button>
            <button
              type="button"
              className="icon-btn"
              title="Delete this note"
              disabled={!currentNote}
              onClick={() => actions.current.deleteCurrent()}
            >
              <Trash size={16} />
            </button>
            <button
              type="button"
              className="icon-btn"
              title="Save (Cmd/Ctrl+S)"
              onClick={() => void actions.current.save()}
            >
              <FloppyDisk size={16} />
            </button>
          </div>
        </header>

        {!currentNote && (
          <div className="empty-main">
            <NoteBlank size={42} />
            <p>Select or create a note to start writing.</p>
            <button type="button" onClick={() => actions.current.newFile()}>
              New note
            </button>
          </div>
        )}

        {currentNote && view === "editor" && (
          <NoteEditor
            key={`${currentPath}:${editorKey}`}
            initialMarkdown={markdown}
            dark={settings.darkMode}
            fullWidth={fullWidth}
            notes={snap.notes}
            onMarkdown={markDirty}
            onEditor={onEditor}
            onError={toast}
          />
        )}

        {currentNote && view === "raw" && (
          <div className={fullWidth ? "raw-frame full-width" : "raw-frame"}>
            <p className="hint">Raw markdown. Changes apply in the block editor when you switch back.</p>
            <textarea
              ref={rawRef}
              value={markdown}
              spellCheck={false}
              onChange={(event) => markDirty(event.target.value)}
            />
          </div>
        )}

        {currentNote && view === "graph" && graphData && (
          <div className="graph-frame">
            <p className="hint">
              {graphData.nodes.length} notes, {graphData.edges.length} links. Click a node to open
              it. Drag to pan, scroll to zoom.
            </p>
            <GraphView
              data={graphData}
              currentPath={currentPath}
              onOpen={(path) => void openNote(path).catch((error) => toast(errorMessage(error)))}
            />
          </div>
        )}
        {currentNote && view === "graph" && !graphData && <div className="boot">Building graph…</div>}
      </main>

      {showSettings && (
        <SettingsDialog
          settings={settings}
          onClose={() => setShowSettings(false)}
          onSave={applySettings}
          onToggleTheme={() => void toggleTheme()}
        />
      )}
      {pendingDelete && (
        <Dialog title="Delete" onClose={() => setPendingDelete(null)}>
          <p>
            Delete “{pendingDelete.name.replace(/\.md$/i, "")}”
            {pendingDelete.isDir ? " and everything inside it" : ""}?
          </p>
          <p className="muted">This cannot be undone.</p>
          <div className="dialog-actions">
            <button type="button" className="danger" onClick={() => void removePath(pendingDelete)}>
              Delete
            </button>
            <button type="button" onClick={() => setPendingDelete(null)}>
              Cancel
            </button>
          </div>
        </Dialog>
      )}
      <div className="toasts">
        {toasts.map((item) => (
          <div key={item.id} className="toast">
            {item.message}
          </div>
        ))}
      </div>
    </div>
  );
}

function readFullWidth() {
  return localStorage.getItem("kbwrite.editorWidth") === "full";
}

function readSidebarWidth() {
  const saved = Number(localStorage.getItem("kbwrite.sidebar"));
  return Number.isFinite(saved) && saved >= 180 && saved <= 480 ? saved : 260;
}

function nextUntitled(tree: Snapshot["tree"]) {
  const taken = new Set(
    tree.filter((node) => !node.isDir).map((node) => node.name.toLowerCase()),
  );
  let name = "Untitled.md";
  let n = 1;
  while (taken.has(name.toLowerCase())) {
    n += 1;
    name = `Untitled-${n}.md`;
  }
  return name.slice(0, -3);
}

function pathInside(path: string, parent: string) {
  if (path === parent) return true;
  const sep = parent.includes("\\") ? "\\" : "/";
  const prefix = parent.endsWith(sep) ? parent : parent + sep;
  return path.startsWith(prefix);
}

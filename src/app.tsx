import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { BlockNoteEditor } from "@blocknote/core";
import {
  ArrowsInLineHorizontal,
  ArrowsOutLineHorizontal,
  BookmarkSimple,
  Code,
  FileText,
  FloppyDisk,
  Graph,
  ListBullets,
  ArrowsIn,
  LockSimple,
  LockSimpleOpen,
  Moon,
  NoteBlank,
  PencilSimple,
  Sun,
  Trash,
  X,
} from "@phosphor-icons/react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  createFile,
  createFolder,
  deletePath,
  errorMessage,
  getSnapshot,
  graph,
  listPlugins,
  openPrivacySettings,
  readNote,
  reloadVault,
  renamePath,
  resolvePluginDialog,
  saveSettings,
  writeNote,
} from "./api";
import { formatMarkdown } from "./format-markdown";
import { CommandPalette, type PaletteCommand, type PaletteMode } from "./components/command-palette";
import { Dialog } from "./components/dialog";
import { GraphView } from "./components/graph-view";
import { NoteEditor } from "./components/note-editor";
import { PluginDialog } from "./components/plugin-dialog";
import { SettingsDialog } from "./components/settings-dialog";
import { Sidebar } from "./components/sidebar";
import type { GraphData, PluginBlock, PluginDialogRequest, Settings, Snapshot, ViewMode } from "./types";

type Toast = { id: number; message: string };
type UpdatePhase = "checking" | "installing" | "restart";
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
  const [pluginBlocks, setPluginBlocks] = useState<PluginBlock[]>([]);
  const [pluginDialogs, setPluginDialogs] = useState<PluginDialogRequest[]>([]);
  const [updatePhase, setUpdatePhase] = useState<UpdatePhase | null>(null);
  const [sidebarWidth, setSidebarWidth] = useState(readSidebarWidth);
  const [fullWidth, setFullWidth] = useState(readFullWidth);
  const [zen, setZen] = useState(() => localStorage.getItem("kbwrite.zen") === "on");
  const [readOnly, setReadOnly] = useState(false);
  const [palette, setPalette] = useState<PaletteMode | null>(null);

  const markdownRef = useRef("");
  const dirtyRef = useRef(false);
  const currentPathRef = useRef<string | null>(null);
  const viewRef = useRef<ViewMode>("editor");
  const editorRef = useRef<BlockNoteEditor<any, any, any> | null>(null);
  const rawRef = useRef<HTMLTextAreaElement>(null);
  const saveQueue = useRef(Promise.resolve());
  const saveTimer = useRef<number | null>(null);
  const toastId = useRef(0);
  const updateBusy = useRef(false);
  const autoFormatRef = useRef(false);
  const actions = useRef({
    save: async () => {},
    newFile: () => {},
    toggleSettings: () => {},
    deleteCurrent: () => {},
    toggleReadOnly: () => {},
    toggleZen: () => {},
    openPalette: (_mode: PaletteMode) => {},
    formatNote: async () => {},
  });

  const toast = useCallback((message: string) => {
    const id = ++toastId.current;
    setToasts((current) => [...current, { id, message }]);
    window.setTimeout(() => {
      setToasts((current) => current.filter((item) => item.id !== id));
    }, 4000);
  }, []);

  const onEditor = useCallback((editor: BlockNoteEditor<any, any, any> | null) => {
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
      }
      if (autoFormatRef.current) {
        content = await formatMarkdown(content);
      }
      markdownRef.current = content;
      setMarkdown(content);
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

  // Push appearance settings into CSS custom properties and inject custom CSS.
  useEffect(() => {
    const root = document.documentElement;
    const systemUi = '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
    const uiStack = settings?.uiFont ? `${settings.uiFont}, ${systemUi}` : systemUi;
    root.style.setProperty("--kb-ui-font-stack", uiStack);
    root.style.setProperty(
      "--kb-editor-font-stack",
      settings?.editorFont ? `${settings.editorFont}, ${systemUi}` : uiStack,
    );
    root.style.setProperty("--kb-editor-font-size", `${settings?.fontSize || 16}px`);
    let style = document.getElementById("kbwrite-custom-css") as HTMLStyleElement | null;
    if (!style) {
      style = document.createElement("style");
      style.id = "kbwrite-custom-css";
      document.head.appendChild(style);
    }
    style.textContent = settings?.customCss || "";
  }, [settings?.uiFont, settings?.editorFont, settings?.fontSize, settings?.customCss]);

  useEffect(() => {
    autoFormatRef.current = settings?.autoFormat ?? false;
  }, [settings?.autoFormat]);

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
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen<string>("plugin-notice", (event) => {
      if (event.payload) toast(event.payload);
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [toast]);

  const refreshPluginBlocks = useCallback(async () => {
    try {
      const plugins = await listPlugins();
      setPluginBlocks(
        plugins.filter((plugin) => plugin.loaded).flatMap((plugin) => plugin.blocks ?? []),
      );
    } catch {
      setPluginBlocks([]);
    }
  }, []);

  useEffect(() => {
    void refreshPluginBlocks();
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen("plugins-changed", () => {
      void refreshPluginBlocks();
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refreshPluginBlocks]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen<PluginDialogRequest>("plugin-dialog", (event) => {
      if (!event.payload) return;
      setPluginDialogs((current) => [...current, event.payload]);
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const checkForUpdate = useCallback(async () => {
    if (updateBusy.current) return;
    updateBusy.current = true;
    setUpdatePhase("checking");
    try {
      const update = await check();
      if (!update) {
        setUpdatePhase(null);
        updateBusy.current = false;
        toast("KbWrite is up to date");
        return;
      }
      setUpdatePhase("installing");
      await update.downloadAndInstall();
      setUpdatePhase("restart");
    } catch (error) {
      setUpdatePhase(null);
      updateBusy.current = false;
      toast(errorMessage(error));
    }
  }, [toast]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen("check-updates", () => {
      void checkForUpdate();
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [checkForUpdate]);

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
      } else if (key === "p") {
        event.preventDefault();
        actions.current.openPalette("commands");
      } else if (key === "o") {
        event.preventDefault();
        actions.current.openPalette("files");
      } else if (key === "f" && event.shiftKey) {
        event.preventDefault();
        actions.current.openPalette("search");
      } else if (key === "e") {
        event.preventDefault();
        actions.current.toggleReadOnly();
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

  const isMac = navigator.userAgent.includes("Mac");
  const mod = isMac ? "⌘" : "Ctrl+";
  const paletteCommands = useMemo<PaletteCommand[]>(() => {
    const bookmarked = currentNote ? settings?.bookmarks.includes(currentNote.path) : false;
    return [
      { id: "new-note", label: "New note", shortcut: `${mod}N`, run: () => actions.current.newFile() },
      { id: "save", label: "Save note", shortcut: `${mod}S`, run: () => void actions.current.save() },
      { id: "format", label: "Format note (Prettier)", run: () => void actions.current.formatNote() },
      {
        id: "read-only",
        label: readOnly ? "Turn off read-only view" : "Turn on read-only view",
        shortcut: `${mod}E`,
        run: () => actions.current.toggleReadOnly(),
      },
      { id: "zen", label: zen ? "Exit zen mode" : "Zen mode", run: () => actions.current.toggleZen() },
      {
        id: "theme",
        label: settings?.darkMode ? "Switch to light mode" : "Switch to dark mode",
        run: () => void toggleTheme(),
      },
      { id: "view-editor", label: "Show editor", run: () => void showView("editor") },
      { id: "view-raw", label: "Raw markdown", run: () => void showView("raw") },
      { id: "view-graph", label: "Graph view", run: () => void showView("graph") },
      {
        id: "bookmark",
        label: currentNote
          ? bookmarked
            ? "Remove bookmark"
            : "Bookmark this note"
          : "Bookmark this note",
        disabled: !currentNote,
        hint: currentNote ? currentNote.title : undefined,
        run: () => {
          if (currentNote) void toggleBookmark(currentNote.path);
        },
      },
      { id: "search", label: "Global search", shortcut: isMac ? "⇧⌘F" : "Ctrl+Shift+F", run: () => setPalette("search") },
      { id: "files", label: "Go to file", shortcut: `${mod}O`, run: () => setPalette("files") },
      {
        id: "settings",
        label: "Settings",
        shortcut: `${mod},`,
        run: () => actions.current.toggleSettings(),
      },
    ];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentNote, settings, readOnly, zen, mod, isMac]);

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

  // Persists a partial settings change (bookmarks, theme) without opening the
  // settings dialog.
  async function updateSettings(patch: Partial<Settings>) {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    try {
      const updated = await saveSettings(next);
      setSnap(updated);
      setSettings(updated.settings);
    } catch (error) {
      toast(errorMessage(error));
    }
  }

  async function toggleBookmark(path: string) {
    if (!settings) return;
    const has = settings.bookmarks.includes(path);
    await updateSettings({
      bookmarks: has
        ? settings.bookmarks.filter((bookmark) => bookmark !== path)
        : [...settings.bookmarks, path],
    });
    toast(has ? "Bookmark removed" : "Bookmarked");
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
    toggleReadOnly: () => setReadOnly((current) => !current),
    toggleZen: () =>
      setZen((current) => {
        localStorage.setItem("kbwrite.zen", current ? "off" : "on");
        return !current;
      }),
    openPalette: (mode: PaletteMode) => setPalette(mode),
    formatNote: async () => {
      try {
        await saveCurrent();
        const formatted = await formatMarkdown(markdownRef.current);
        if (formatted === markdownRef.current) {
          toast("Note is already formatted");
          return;
        }
        markdownRef.current = formatted;
        setMarkdown(formatted);
        setDirty(true);
        if (viewRef.current === "editor") setEditorKey((key) => key + 1);
        toast("Note formatted");
      } catch {
        /* toasted in saveCurrent */
      }
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
    <div className={`shell${zen ? " zen" : ""}${readOnly ? " read-only" : ""}`}>
      <div className="sidebar-slot" style={{ width: sidebarWidth }}>
        <Sidebar
          key={settings.vaultPath}
          vaultPath={settings.vaultPath}
          tree={snap.tree}
          currentPath={currentPath}
          bookmarks={settings.bookmarks}
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
          onToggleBookmark={(path) => void toggleBookmark(path)}
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
            <div className="segmented" role="tablist">
              <button
                type="button"
                className={view === "editor" ? "active" : ""}
                data-tooltip="Editor"
                onClick={() => void showView("editor")}
              >
                <PencilSimple size={14} /> Editor
              </button>
              <button
                type="button"
                className={view === "raw" ? "active" : ""}
                data-tooltip="Raw markdown"
                onClick={() => void showView("raw")}
              >
                <Code size={14} /> Raw
              </button>
              <button
                type="button"
                className={view === "graph" ? "active" : ""}
                data-tooltip="Graph"
                onClick={() => void showView("graph")}
              >
                <Graph size={14} /> Graph
              </button>
            </div>
            {(view === "editor" || view === "raw") && (
              <button
                type="button"
                className="icon-btn"
                data-tooltip={fullWidth ? "Reduced width" : "Full width"}
                aria-label={fullWidth ? "Reduced width" : "Full width"}
                aria-pressed={fullWidth}
                onClick={() => chooseWidth(!fullWidth)}
              >
                {fullWidth ? <ArrowsInLineHorizontal size={16} /> : <ArrowsOutLineHorizontal size={16} />}
              </button>
            )}
            <button
              type="button"
              className="icon-btn"
              data-tooltip={readOnly ? "Read-only on (Cmd/Ctrl+E)" : "Make editable (Cmd/Ctrl+E)"}
              aria-label={readOnly ? "Read-only on" : "Make editable"}
              aria-pressed={readOnly}
              onClick={() => actions.current.toggleReadOnly()}
            >
              {readOnly ? <LockSimple size={16} /> : <LockSimpleOpen size={16} />}
            </button>
            <button
              type="button"
              className="icon-btn"
              data-tooltip="Command palette (Cmd/Ctrl+P)"
              aria-label="Command palette"
              onClick={() => setPalette("commands")}
            >
              <ListBullets size={16} />
            </button>
            <button
              type="button"
              className="icon-btn"
              data-tooltip={settings.darkMode ? "Switch to light mode" : "Switch to dark mode"}
              aria-label={settings.darkMode ? "Switch to light mode" : "Switch to dark mode"}
              onClick={() => void toggleTheme()}
            >
              {settings.darkMode ? <Sun size={16} /> : <Moon size={16} />}
            </button>
            <button
              type="button"
              className="icon-btn"
              data-tooltip={currentNote && settings.bookmarks.includes(currentNote.path) ? "Remove bookmark" : "Bookmark this note"}
              aria-label="Toggle bookmark"
              disabled={!currentNote}
              onClick={() => currentNote && void toggleBookmark(currentNote.path)}
            >
              {currentNote && settings.bookmarks.includes(currentNote.path) ? (
                <BookmarkSimple size={16} weight="fill" />
              ) : (
                <BookmarkSimple size={16} />
              )}
            </button>
            <button
              type="button"
              className="icon-btn"
              data-tooltip="Zen mode"
              aria-label="Zen mode"
              aria-pressed={zen}
              onClick={() => actions.current.toggleZen()}
            >
              <ArrowsIn size={16} />
            </button>
            <button
              type="button"
              className="icon-btn"
              data-tooltip="Delete this note"
              aria-label="Delete this note"
              disabled={!currentNote}
              onClick={() => actions.current.deleteCurrent()}
            >
              <Trash size={16} />
            </button>
            <button
              type="button"
              className="icon-btn"
              data-tooltip="Save (Cmd/Ctrl+S)"
              aria-label="Save"
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
            key={`${currentPath}:${editorKey}:${pluginBlocks.map((block) => block.type).join(",")}`}
            notePath={currentPath ?? ""}
            initialMarkdown={markdown}
            dark={settings.darkMode}
            fullWidth={fullWidth}
            editable={!readOnly}
            notes={snap.notes}
            pluginBlocks={pluginBlocks}
            onMarkdown={markDirty}
            onEditor={onEditor}
            onError={toast}
            onOpen={(path) => void openNote(path).catch((error) => toast(errorMessage(error)))}
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

      {zen && (
        <button
          type="button"
          className="zen-exit"
          data-tooltip="Exit zen mode"
          aria-label="Exit zen mode"
          onClick={() => actions.current.toggleZen()}
        >
          <X size={14} />
        </button>
      )}

      {palette && (
        <CommandPalette
          mode={palette}
          commands={paletteCommands}
          notes={snap.notes}
          bookmarks={settings.bookmarks}
          currentPath={currentPath}
          onClose={() => setPalette(null)}
          onOpenNote={(path) => void openNote(path).catch((error) => toast(errorMessage(error)))}
          onModeChange={setPalette}
        />
      )}

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
      {pluginDialogs[0] && (
        <PluginDialog
          key={pluginDialogs[0].id}
          request={pluginDialogs[0]}
          onClose={(response) => {
            const id = pluginDialogs[0].id;
            setPluginDialogs((current) => current.filter((item) => item.id !== id));
            void resolvePluginDialog(id, response).catch((error) => toast(errorMessage(error)));
          }}
        />
      )}
      <div className="toasts">
        {toasts.map((item) => (
          <div key={item.id} className="toast">
            {item.message}
          </div>
        ))}
        {updatePhase && (
          <div className="toast update-toast" role="status">
            {updatePhase !== "restart" && <span className="spinner" aria-hidden="true" />}
            <span>
              {updatePhase === "checking" && "Checking for updates"}
              {updatePhase === "installing" && "Installing update"}
              {updatePhase === "restart" && "Update ready"}
            </span>
            {updatePhase === "restart" && (
              <button
                type="button"
                className="primary"
                onClick={() => void relaunch().catch((error) => toast(errorMessage(error)))}
              >
                Restart
              </button>
            )}
          </div>
        )}
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

import { useEffect, useRef, useState, type MouseEvent } from "react";
import {
  CaretDown,
  CaretRight,
  FilePlus,
  FileText,
  Folder,
  FolderOpen,
  FolderPlus,
  GearSix,
  Info,
  MagnifyingGlass,
  Notebook,
  PencilSimple,
  Trash,
  Warning,
} from "@phosphor-icons/react";
import { errorMessage } from "../api";
import type { TreeNode } from "../types";

type EditState =
  | { kind: "new-file" | "new-folder"; dir: string }
  | { kind: "rename"; dir: string; target: string; isDir: boolean };

type MenuState = {
  x: number;
  y: number;
  path: string;
  name: string;
  isDir: boolean;
};

function labelOf(name: string, isDir: boolean) {
  return isDir ? name : name.replace(/\.md$/i, "");
}

export function Sidebar({
  vaultPath,
  tree,
  currentPath,
  vaultError,
  walkErrors,
  permissionDenied,
  onOpen,
  onCreateFile,
  onCreateFolder,
  onRename,
  onDelete,
  onOpenSettings,
  onRetry,
  onPrivacy,
}: {
  vaultPath: string;
  tree: TreeNode[];
  currentPath: string | null;
  vaultError: string | null;
  walkErrors: string[];
  permissionDenied: boolean;
  onOpen: (path: string) => void;
  onCreateFile: (dir: string, name: string) => Promise<void>;
  onCreateFolder: (dir: string, name: string) => Promise<void>;
  onRename: (path: string, name: string) => Promise<void>;
  onDelete: (node: { path: string; name: string; isDir: boolean }) => void;
  onOpenSettings: () => void;
  onRetry: () => void;
  onPrivacy: () => void;
}) {
  const [search, setSearch] = useState("");
  const [openDirs, setOpenDirs] = useState<Set<string>>(() => collectDirs(tree));
  const [editing, setEditing] = useState<EditState | null>(null);
  const [editName, setEditName] = useState("");
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const close = () => setMenu(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, []);

  function startCreate(kind: "new-file" | "new-folder", dir: string) {
    setOpenDirs((dirs) => new Set(dirs).add(dir));
    setEditing({ kind, dir });
    setEditName("");
    setError(null);
    setMenu(null);
  }

  function startRename(node: { path: string; name: string; isDir: boolean }) {
    const parent = node.path.slice(0, Math.max(0, node.path.length - node.name.length - 1));
    setEditing({
      kind: "rename",
      dir: parent || vaultPath,
      target: node.path,
      isDir: node.isDir,
    });
    setEditName(labelOf(node.name, node.isDir));
    setError(null);
    setMenu(null);
  }

  async function commitEdit() {
    if (!editing) return;
    const name = editName.trim();
    if (!name) {
      setEditing(null);
      return;
    }
    try {
      if (editing.kind === "rename") await onRename(editing.target, name);
      else if (editing.kind === "new-file") await onCreateFile(editing.dir, name);
      else await onCreateFolder(editing.dir, name);
      setEditing(null);
      setError(null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const query = search.trim().toLowerCase();
  const flat = query ? flatten(tree).filter((n) => n.rel.toLowerCase().includes(query)) : [];

  return (
    <aside className="sidebar">
      <div className="sidebar-head">
        <div className="brand">
          <Notebook size={18} weight="duotone" />
          <span>KbWrite</span>
        </div>
        <div className="sidebar-actions">
          <button
            type="button"
            className="icon-btn"
            title="New file (Cmd/Ctrl+N)"
            onClick={() => startCreate("new-file", vaultPath)}
          >
            <FilePlus size={16} />
          </button>
          <button
            type="button"
            className="icon-btn"
            title="New folder"
            onClick={() => startCreate("new-folder", vaultPath)}
          >
            <FolderPlus size={16} />
          </button>
          <button
            type="button"
            className="icon-btn"
            title="Settings (Cmd/Ctrl+,)"
            onClick={onOpenSettings}
          >
            <GearSix size={16} />
          </button>
        </div>
      </div>

      <label className="search">
        <MagnifyingGlass size={14} />
        <input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Search notes"
          aria-label="Search notes"
        />
      </label>

      <div
        className="tree-scroll"
        onContextMenu={(event) => {
          if (query) return;
          event.preventDefault();
          setMenu({
            x: event.clientX,
            y: event.clientY,
            path: vaultPath,
            name: "",
            isDir: true,
          });
        }}
      >
        {query ? (
          flat.length === 0 ? (
            <p className="empty-copy">No notes match “{query}”.</p>
          ) : (
            flat.map((note) => (
              <button
                key={note.path}
                type="button"
                className={note.path === currentPath ? "tree-row current" : "tree-row"}
                onClick={() => onOpen(note.path)}
              >
                <FileText size={15} />
                <span>{note.rel.replace(/\.md$/i, "")}</span>
              </button>
            ))
          )
        ) : (
          <>
            {editing && editing.kind !== "rename" && editing.dir === vaultPath && (
              <EditRow
                depth={0}
                isDir={editing.kind === "new-folder"}
                value={editName}
                onChange={setEditName}
                onCommit={() => void commitEdit()}
                onCancel={() => setEditing(null)}
              />
            )}
            <Tree
              nodes={tree}
              depth={0}
              currentPath={currentPath}
              openDirs={openDirs}
              editing={editing}
              editName={editName}
              onToggle={(path) =>
                setOpenDirs((dirs) => {
                  const next = new Set(dirs);
                  if (next.has(path)) next.delete(path);
                  else next.add(path);
                  return next;
                })
              }
              onOpen={onOpen}
              onMenu={(event, node) => {
                event.preventDefault();
                event.stopPropagation();
                setMenu({
                  x: event.clientX,
                  y: event.clientY,
                  path: node.path,
                  name: node.name,
                  isDir: node.isDir,
                });
              }}
              onEditName={setEditName}
              onCommit={() => void commitEdit()}
              onCancel={() => setEditing(null)}
            />
            {tree.length === 0 && !editing && (
              <EmptyVault
                vaultError={vaultError}
                walkErrors={walkErrors}
                permissionDenied={permissionDenied}
                vaultPath={vaultPath}
                onRetry={onRetry}
                onPrivacy={onPrivacy}
              />
            )}
          </>
        )}
      </div>

      {error && <p className="sidebar-error">{error}</p>}

      <footer className="sidebar-foot">
        {(vaultError || walkErrors[0]) && (
          <p className="sidebar-error">
            <Warning size={12} />
            {vaultError || walkErrors[0]}
          </p>
        )}
        <button type="button" className="vault-path" onClick={onOpenSettings} title={vaultPath}>
          <FolderOpen size={12} />
          <span>{shortPath(vaultPath)}</span>
        </button>
      </footer>

      {menu && (
        <div
          className="context-menu"
          style={{ left: menu.x, top: menu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          {menu.isDir && (
            <>
              <button
                type="button"
                onClick={() => startCreate("new-file", menu.path)}
              >
                New file
              </button>
              <button
                type="button"
                onClick={() => startCreate("new-folder", menu.path)}
              >
                New folder
              </button>
            </>
          )}
          {menu.name && (
            <>
              <button
                type="button"
                onClick={() =>
                  startRename({ path: menu.path, name: menu.name, isDir: menu.isDir })
                }
              >
                <PencilSimple size={14} />
                Edit
              </button>
              <button
                type="button"
                className="danger-text"
                onClick={() => {
                  onDelete({ path: menu.path, name: menu.name, isDir: menu.isDir });
                  setMenu(null);
                }}
              >
                <Trash size={14} />
                Delete
              </button>
            </>
          )}
        </div>
      )}
    </aside>
  );
}

function Tree({
  nodes,
  depth,
  currentPath,
  openDirs,
  editing,
  editName,
  onToggle,
  onOpen,
  onMenu,
  onEditName,
  onCommit,
  onCancel,
}: {
  nodes: TreeNode[];
  depth: number;
  currentPath: string | null;
  openDirs: Set<string>;
  editing: EditState | null;
  editName: string;
  onToggle: (path: string) => void;
  onOpen: (path: string) => void;
  onMenu: (event: MouseEvent, node: TreeNode) => void;
  onEditName: (value: string) => void;
  onCommit: () => void;
  onCancel: () => void;
}) {
  return (
    <>
      {nodes.map((node) => {
        const renaming =
          editing?.kind === "rename" && editing.target === node.path;
        const open = node.isDir && openDirs.has(node.path);
        return (
          <div key={node.path}>
            {renaming ? (
              <EditRow
                depth={depth}
                isDir={node.isDir}
                value={editName}
                onChange={onEditName}
                onCommit={onCommit}
                onCancel={onCancel}
              />
            ) : (
              <button
                type="button"
                className={node.path === currentPath ? "tree-row current" : "tree-row"}
                style={{ paddingLeft: 10 + depth * 14 }}
                onClick={() => (node.isDir ? onToggle(node.path) : onOpen(node.path))}
                onContextMenu={(event) => onMenu(event, node)}
              >
                {node.isDir ? (
                  open ? (
                    <CaretDown size={12} />
                  ) : (
                    <CaretRight size={12} />
                  )
                ) : (
                  <span className="tree-spacer" />
                )}
                {node.isDir ? (
                  open ? (
                    <FolderOpen size={15} />
                  ) : (
                    <Folder size={15} />
                  )
                ) : (
                  <FileText size={15} />
                )}
                <span>{labelOf(node.name, node.isDir)}</span>
              </button>
            )}
            {open && (
              <>
                {editing &&
                  editing.kind !== "rename" &&
                  editing.dir === node.path && (
                    <EditRow
                      depth={depth + 1}
                      isDir={editing.kind === "new-folder"}
                      value={editName}
                      onChange={onEditName}
                      onCommit={onCommit}
                      onCancel={onCancel}
                    />
                  )}
                <Tree
                  nodes={node.children}
                  depth={depth + 1}
                  currentPath={currentPath}
                  openDirs={openDirs}
                  editing={editing}
                  editName={editName}
                  onToggle={onToggle}
                  onOpen={onOpen}
                  onMenu={onMenu}
                  onEditName={onEditName}
                  onCommit={onCommit}
                  onCancel={onCancel}
                />
              </>
            )}
          </div>
        );
      })}
    </>
  );
}

function EditRow({
  depth,
  isDir,
  value,
  onChange,
  onCommit,
  onCancel,
}: {
  depth: number;
  isDir: boolean;
  value: string;
  onChange: (value: string) => void;
  onCommit: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  return (
    <div className="tree-edit" style={{ paddingLeft: 10 + depth * 14 }}>
      {isDir ? <Folder size={15} /> : <FileText size={15} />}
      <input
        ref={ref}
        value={value}
        placeholder={isDir ? "Folder name" : "File name"}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            onCommit();
          }
          if (event.key === "Escape") {
            event.preventDefault();
            onCancel();
          }
        }}
        onBlur={onCommit}
      />
    </div>
  );
}

function EmptyVault({
  vaultError,
  walkErrors,
  permissionDenied,
  vaultPath,
  onRetry,
  onPrivacy,
}: {
  vaultError: string | null;
  walkErrors: string[];
  permissionDenied: boolean;
  vaultPath: string;
  onRetry: () => void;
  onPrivacy: () => void;
}) {
  if (permissionDenied) {
    return (
      <div className="empty-copy">
        <p>
          <Warning size={14} /> Access to the notes folder is blocked by macOS.
        </p>
        <p>Allow KbWrite to access {vaultPath}.</p>
        <div className="empty-actions">
          <button type="button" onClick={onPrivacy}>
            Open System Settings
          </button>
          <button type="button" onClick={onRetry}>
            Try again
          </button>
        </div>
      </div>
    );
  }
  if (vaultError || walkErrors[0]) {
    return (
      <div className="empty-copy">
        <p>{vaultError || walkErrors[0]}</p>
        <button type="button" onClick={onRetry}>
          Try again
        </button>
      </div>
    );
  }
  return (
    <p className="empty-copy">
      <Info size={14} /> No notes yet. Use New file or right-click here.
    </p>
  );
}

function collectDirs(nodes: TreeNode[], into = new Set<string>()) {
  for (const node of nodes) {
    if (node.isDir) {
      into.add(node.path);
      collectDirs(node.children, into);
    }
  }
  return into;
}

function flatten(nodes: TreeNode[], into: { path: string; rel: string }[] = [], prefix = "") {
  for (const node of nodes) {
    const rel = prefix ? `${prefix}/${node.name}` : node.name;
    if (node.isDir) flatten(node.children, into, rel);
    else into.push({ path: node.path, rel });
  }
  return into;
}

function shortPath(path: string) {
  return path.length > 42 ? `…${path.slice(-40)}` : path;
}

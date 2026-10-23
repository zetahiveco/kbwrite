import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Moon, Sun } from "@phosphor-icons/react";
import { errorMessage, mcpStart, mcpStatus, mcpStop } from "../api";
import type { McpClient, McpStatus, Settings } from "../types";
import { Dialog } from "./dialog";

const CLIENTS: { id: McpClient; label: string }[] = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "opencode", label: "OpenCode" },
];

export function SettingsDialog({
  settings,
  onClose,
  onSave,
  onToggleTheme,
}: {
  settings: Settings;
  onClose: () => void;
  onSave: (settings: Settings) => Promise<void>;
  onToggleTheme: () => void;
}) {
  const [draft, setDraft] = useState(settings);
  const [busy, setBusy] = useState(false);
  const [mcp, setMcp] = useState<McpStatus | null>(null);
  const [mcpError, setMcpError] = useState<string | null>(null);
  const [mcpBusy, setMcpBusy] = useState(false);
  const [client, setClient] = useState<McpClient>("claude");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void mcpStatus()
      .then((status) => {
        if (!cancelled) setMcp(status);
      })
      .catch((error) => {
        if (!cancelled) setMcpError(errorMessage(error));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function setServer(running: boolean) {
    setMcpBusy(true);
    setMcpError(null);
    try {
      setMcp(await (running ? mcpStart() : mcpStop()));
    } catch (error) {
      setMcpError(errorMessage(error));
    } finally {
      setMcpBusy(false);
    }
  }

  async function copyCommand() {
    if (!mcp) return;
    const text = mcp[client];
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const area = document.createElement("textarea");
      area.value = text;
      area.setAttribute("readonly", "");
      area.style.position = "absolute";
      area.style.left = "-9999px";
      document.body.appendChild(area);
      area.select();
      document.execCommand("copy");
      document.body.removeChild(area);
    }
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  }

  async function browse() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Choose vault folder",
    });
    if (typeof picked === "string") {
      setDraft((current) => ({ ...current, vaultPath: picked }));
    }
  }

  return (
    <Dialog title="Settings" onClose={onClose} wide>
      <section>
        <h3>Appearance</h3>
        <button type="button" className="text-btn" onClick={onToggleTheme}>
          {settings.darkMode ? <Moon size={15} /> : <Sun size={15} />}
          {settings.darkMode ? "Dark mode" : "Light mode"}
        </button>
      </section>
      <section>
        <h3>Vault</h3>
        <div className="field-row">
          <input
            className="field"
            value={draft.vaultPath}
            onChange={(event) =>
              setDraft((current) => ({ ...current, vaultPath: event.target.value }))
            }
            aria-label="Vault folder"
          />
          <button type="button" onClick={() => void browse()}>
            Browse
          </button>
        </div>
      </section>
      <section>
        <h3>MCP server</h3>
        <p className="muted">
          Start it, then add KbWrite to Claude, Codex, or OpenCode. It can read and write this vault
          only while KbWrite is open.
        </p>
        <div className="mcp-status">
          <span>{mcp?.running ? `Running at ${mcp.url}` : "Stopped"}</span>
          {mcp?.running ? (
            <button type="button" disabled={mcpBusy} onClick={() => void setServer(false)}>
              Stop
            </button>
          ) : (
            <button
              type="button"
              className="primary"
              disabled={mcpBusy || !mcp}
              onClick={() => void setServer(true)}
            >
              Start
            </button>
          )}
        </div>
        {mcpError && <p className="muted">{mcpError}</p>}
        <div className="segmented" role="tablist" aria-label="MCP client">
          {CLIENTS.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={client === item.id}
              className={client === item.id ? "active" : ""}
              onClick={() => {
                setClient(item.id);
                setCopied(false);
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
        <pre className="mcp-cmd">{mcp ? mcp[client] : "Loading the command…"}</pre>
        <div className="dialog-actions mcp-copy">
          <button type="button" disabled={!mcp} onClick={() => void copyCommand()}>
            {copied ? "Copied" : "Copy"}
          </button>
        </div>
      </section>
      <div className="dialog-actions">
        <button
          type="button"
          className="primary"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            void onSave({ ...draft, darkMode: settings.darkMode }).finally(() => setBusy(false));
          }}
        >
          Save settings
        </button>
      </div>
    </Dialog>
  );
}

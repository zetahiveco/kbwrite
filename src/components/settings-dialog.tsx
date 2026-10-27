import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Moon, Sun } from "@phosphor-icons/react";
import {
  errorMessage,
  installPluginFolder,
  installPluginGithub,
  listPlugins,
  mcpStart,
  mcpStatus,
  mcpStop,
  removePlugin,
  runPluginCommand,
  setPluginEnabled,
} from "../api";
import type { McpClient, McpStatus, PluginInfo, Settings } from "../types";
import { Dialog } from "./dialog";

const CLIENTS: { id: McpClient; label: string }[] = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "opencode", label: "OpenCode" },
];

const UI_FONTS = [
  { value: "", label: "System default" },
  { value: "'SF Pro Text', ui-sans-serif, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif", label: "Sans — system" },
  { value: "'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, serif", label: "Serif" },
  { value: "'Avenir Next', 'Helvetica Neue', Arial, sans-serif", label: "Avenir" },
  { value: "'SF Mono', ui-monospace, Menlo, Consolas, monospace", label: "Monospace" },
];

const EDITOR_FONTS = [
  { value: "", label: "Same as interface" },
  { value: "'SF Mono', ui-monospace, Menlo, Consolas, 'Courier New', monospace", label: "Monospace" },
  { value: "'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, serif", label: "Serif" },
  { value: "'SF Pro Text', ui-sans-serif, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif", label: "Sans — system" },
];

const FONT_SIZES = [13, 14, 15, 16, 18, 20, 24];

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
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [pluginError, setPluginError] = useState<string | null>(null);
  const [pluginBusy, setPluginBusy] = useState(false);
  const [githubUrl, setGithubUrl] = useState("");

  useEffect(() => {
    let cancelled = false;
    void mcpStatus()
      .then((status) => {
        if (!cancelled) setMcp(status);
      })
      .catch((error) => {
        if (!cancelled) setMcpError(errorMessage(error));
      });
    void listPlugins()
      .then((loaded) => {
        if (!cancelled) setPlugins(loaded);
      })
      .catch((error) => {
        if (!cancelled) setPluginError(errorMessage(error));
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

  async function changePlugins(action: () => Promise<PluginInfo[]>) {
    setPluginBusy(true);
    setPluginError(null);
    try {
      setPlugins(await action());
    } catch (error) {
      setPluginError(errorMessage(error));
    } finally {
      setPluginBusy(false);
    }
  }

  async function installFolder() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Choose plugin folder",
    });
    if (typeof picked === "string") {
      await changePlugins(() => installPluginFolder(picked));
    }
  }

  async function installGithub() {
    const url = githubUrl.trim();
    if (!url) return;
    await changePlugins(() => installPluginGithub(url));
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
        <div className="field-grid">
          <label>
            Interface font
            <select
              value={UI_FONTS.some((font) => font.value === draft.uiFont) ? draft.uiFont : "custom"}
              onChange={(event) =>
                setDraft((current) => ({ ...current, uiFont: event.target.value === "custom" ? current.uiFont : event.target.value }))
              }
            >
              {UI_FONTS.map((font) => (
                <option key={font.label} value={font.value}>
                  {font.label}
                </option>
              ))}
              {draft.uiFont && !UI_FONTS.some((font) => font.value === draft.uiFont) && (
                <option value="custom">Custom</option>
              )}
            </select>
          </label>
          <label>
            Editor font
            <select
              value={EDITOR_FONTS.some((font) => font.value === draft.editorFont) ? draft.editorFont : "custom"}
              onChange={(event) =>
                setDraft((current) => ({ ...current, editorFont: event.target.value === "custom" ? current.editorFont : event.target.value }))
              }
            >
              {EDITOR_FONTS.map((font) => (
                <option key={font.label} value={font.value}>
                  {font.label}
                </option>
              ))}
              {draft.editorFont && !EDITOR_FONTS.some((font) => font.value === draft.editorFont) && (
                <option value="custom">Custom</option>
              )}
            </select>
          </label>
          <label>
            Editor font size
            <select
              value={String(draft.fontSize)}
              onChange={(event) =>
                setDraft((current) => ({ ...current, fontSize: Number(event.target.value) }))
              }
            >
              {FONT_SIZES.map((size) => (
                <option key={size} value={String(size)}>
                  {size}px
                </option>
              ))}
            </select>
          </label>
        </div>
      </section>
      <section>
        <h3>Editing</h3>
        <label className="check-row">
          <input
            type="checkbox"
            checked={draft.autoFormat}
            onChange={(event) =>
              setDraft((current) => ({ ...current, autoFormat: event.target.checked }))
            }
          />
          Auto-format markdown on save (Prettier)
        </label>
      </section>
      <section>
        <h3>Custom CSS</h3>
        <p className="muted">
          Applied on top of the built-in theme. Example: <code>.bn-block-content {"{"} letter-spacing: 0.02em; {"}"}</code>
        </p>
        <textarea
          className="css-input"
          rows={5}
          value={draft.customCss}
          spellCheck={false}
          placeholder={"/* your CSS */\n.editor-frame { max-width: 46em; }"}
          onChange={(event) =>
            setDraft((current) => ({ ...current, customCss: event.target.value }))
          }
          aria-label="Custom CSS"
        />
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
      <section>
        <h3>Plugins</h3>
        <p className="muted">
          Rust plugins with a manifest.toml. They run inside KbWrite and can read and write this vault.
          Docs: kbwrite.ink/plugins
        </p>
        {plugins.length === 0 && <p className="muted">No plugins installed.</p>}
        <ul className="plugin-list">
          {plugins.map((plugin) => (
            <li key={plugin.id} className="plugin-row">
              <div className="plugin-title">
                <strong>{plugin.name}</strong>
                <span className="muted">{plugin.version}</span>
              </div>
              {plugin.description && <p>{plugin.description}</p>}
              {plugin.authors && <p className="muted">{plugin.authors}</p>}
              {plugin.error && <p className="plugin-error">{plugin.error}</p>}
              <div className="plugin-actions">
                <button
                  type="button"
                  disabled={pluginBusy}
                  onClick={() => void changePlugins(() => setPluginEnabled(plugin.id, !plugin.enabled))}
                >
                  {plugin.enabled ? "Disable" : "Enable"}
                </button>
                <button
                  type="button"
                  disabled={pluginBusy}
                  onClick={() => void changePlugins(() => removePlugin(plugin.id))}
                >
                  Remove
                </button>
                {plugin.commands.map((command) => (
                  <button
                    key={command.id}
                    type="button"
                    className="primary"
                    disabled={pluginBusy || !plugin.loaded}
                    onClick={() =>
                      void changePlugins(async () => {
                        await runPluginCommand(plugin.id, command.id);
                        return listPlugins();
                      })
                    }
                  >
                    {command.name}
                  </button>
                ))}
              </div>
            </li>
          ))}
        </ul>
        {pluginError && <p className="plugin-error">{pluginError}</p>}
        <div className="plugin-actions">
          <button type="button" disabled={pluginBusy} onClick={() => void installFolder()}>
            Install from folder
          </button>
        </div>
        <label>
          GitHub URL
          <div className="field-row">
            <input
              className="field"
              value={githubUrl}
              placeholder="https://github.com/owner/repo"
              onChange={(event) => setGithubUrl(event.target.value)}
              aria-label="Plugin GitHub URL"
            />
            <button
              type="button"
              className="primary"
              disabled={pluginBusy || !githubUrl.trim()}
              onClick={() => void installGithub()}
            >
              Install
            </button>
          </div>
        </label>
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

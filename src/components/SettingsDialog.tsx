import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Moon, Sun } from "@phosphor-icons/react";
import type { Settings } from "../types";
import { Dialog } from "./Dialog";

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
        <h3>Inline AI</h3>
        <p className="muted">
          OpenAI-compatible endpoint. Works with OpenAI, Ollama, LM Studio, and OpenRouter.
        </p>
        <label>
          Base URL
          <input
            className="field"
            value={draft.openaiBaseUrl}
            onChange={(event) =>
              setDraft((current) => ({ ...current, openaiBaseUrl: event.target.value }))
            }
            placeholder="https://api.openai.com/v1"
          />
        </label>
        <label>
          Model
          <input
            className="field"
            value={draft.openaiModel}
            onChange={(event) =>
              setDraft((current) => ({ ...current, openaiModel: event.target.value }))
            }
            placeholder="gpt-4o-mini"
          />
        </label>
        <label>
          API key
          <input
            className="field"
            type="password"
            value={draft.openaiApiKey}
            onChange={(event) =>
              setDraft((current) => ({ ...current, openaiApiKey: event.target.value }))
            }
            placeholder="sk-…"
          />
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

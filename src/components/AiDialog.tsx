import { useState } from "react";
import { Sparkle } from "@phosphor-icons/react";
import { Dialog } from "./Dialog";

export function AiDialog({
  model,
  noteTitle,
  busy,
  error,
  result,
  onClose,
  onRun,
}: {
  model: string;
  noteTitle: string;
  busy: boolean;
  error: string | null;
  result: string;
  onClose: () => void;
  onRun: (instruction: string) => void;
}) {
  const [instruction, setInstruction] = useState("");

  return (
    <Dialog title="AI Assist" onClose={onClose} wide>
      <p className="muted">
        Model: {model || "not set"} · {noteTitle || "No note"}
      </p>
      <input
        className="field"
        autoFocus
        value={instruction}
        placeholder="Fix grammar, summarize, turn into bullets…"
        onChange={(event) => setInstruction(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !busy && instruction.trim()) {
            event.preventDefault();
            onRun(instruction.trim());
          }
        }}
      />
      <div className="dialog-actions">
        <button
          type="button"
          className="primary"
          disabled={busy || !instruction.trim()}
          onClick={() => onRun(instruction.trim())}
        >
          <Sparkle size={14} />
          {busy ? "Thinking…" : "Run"}
        </button>
        {result && (
          <button
            type="button"
            onClick={() => void navigator.clipboard.writeText(result)}
          >
            Copy result
          </button>
        )}
      </div>
      {error && <p className="form-error">{error}</p>}
      {result && (
        <div className="ai-result">
          <pre>{result}</pre>
          <p className="muted">Inserted below the block you were editing.</p>
        </div>
      )}
    </Dialog>
  );
}

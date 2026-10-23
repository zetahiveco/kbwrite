import { useState } from "react";
import { Dialog } from "./dialog";
import type { PluginDialogRequest } from "../types";

export function PluginDialog({
  request,
  onClose,
}: {
  request: PluginDialogRequest;
  onClose: (response: string) => void;
}) {
  const [values, setValues] = useState<Record<string, string>>(() => {
    const initial: Record<string, string> = {};
    for (const field of request.fields) initial[field.id] = field.value;
    return initial;
  });

  function finish(confirmed: boolean) {
    onClose(JSON.stringify({ confirmed, values: confirmed ? values : {} }));
  }

  return (
    <Dialog title={request.title} onClose={() => finish(false)}>
      {request.message && <p>{request.message}</p>}
      {request.fields.map((field) =>
        field.kind === "checkbox" ? (
          <label key={field.id} className="check-row">
            <input
              type="checkbox"
              checked={values[field.id] === "true"}
              onChange={(event) =>
                setValues((current) => ({
                  ...current,
                  [field.id]: event.target.checked ? "true" : "false",
                }))
              }
            />
            {field.label}
          </label>
        ) : (
          <label key={field.id}>
            {field.label}
            <input
              className="field"
              type={field.kind === "password" ? "password" : "text"}
              value={values[field.id] ?? ""}
              autoComplete={field.kind === "password" ? "off" : undefined}
              onChange={(event) =>
                setValues((current) => ({ ...current, [field.id]: event.target.value }))
              }
            />
          </label>
        ),
      )}
      <div className="dialog-actions">
        <button type="button" onClick={() => finish(false)}>
          {request.cancel}
        </button>
        <button type="button" className="primary" onClick={() => finish(true)}>
          {request.confirm}
        </button>
      </div>
    </Dialog>
  );
}

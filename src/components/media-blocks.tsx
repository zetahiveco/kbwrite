import { useEffect, useState } from "react";
import { FilePanelExtension, type BlockNoteEditor } from "@blocknote/core";
import { createReactBlockSpec } from "@blocknote/react";
import { FileCsv, FileDoc, FilePdf, FileXls } from "@phosphor-icons/react";
import { errorMessage, openMedia, readMediaText } from "../api";
import { displayUrl, fileNameFromUrl, mediaKind, type MediaKind } from "../media";

const props = {
  name: { default: "" },
  url: { default: "" },
};

function addDocumentBlock(type: "pdf" | "csv" | "docx" | "xlsx", accept: string[]) {
  return createReactBlockSpec(
    {
      type,
      propSchema: props,
      content: "none",
    },
    {
      meta: { fileBlockAccept: accept },
      runsBefore: ["file"],
      parse: (element) => parseDocument(element, type),
      render: ({ block, editor }) => (
        <DocumentView block={block} editor={editor as BlockNoteEditor<any, any, any>} kind={type} />
      ),
      toExternalHTML: ({ block }) => (
        <a href={block.props.url}>{block.props.name || fileNameFromUrl(block.props.url)}</a>
      ),
    },
  );
}

function parseDocument(element: HTMLElement, kind: MediaKind) {
  const embed = element.tagName === "EMBED" ? element : element.querySelector("embed");
  if (!embed) return undefined;
  const url = embed.getAttribute("src") || "";
  const marked = embed.getAttribute("data-kbwrite");
  if (marked !== kind && mediaKind(url) !== kind) return undefined;
  return {
    url,
    name: embed.getAttribute("data-name") || fileNameFromUrl(url),
  };
}

export const pdfBlock = addDocumentBlock("pdf", [".pdf", "application/pdf"]);
export const csvBlock = addDocumentBlock("csv", [".csv", "text/csv"]);
export const docxBlock = addDocumentBlock("docx", [
  ".docx",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
]);
export const xlsxBlock = addDocumentBlock("xlsx", [
  ".xlsx",
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
]);

function DocumentView({
  block,
  editor,
  kind,
}: {
  block: { id: string; props: { url: string; name: string } };
  editor: BlockNoteEditor<any, any, any>;
  kind: "pdf" | "csv" | "docx" | "xlsx";
}) {
  const url = block.props.url;
  const name = block.props.name || (url ? fileNameFromUrl(url) : kind.toUpperCase());
  const [src, setSrc] = useState("");
  const [rows, setRows] = useState<string[][] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!url) return;
    let cancelled = false;
    setSrc(editor.resolveFileUrl ? "" : displayUrl(url));
    void editor.resolveFileUrl?.(url).then((resolved) => {
      if (!cancelled) setSrc(resolved);
    });
    if (kind === "csv") {
      void readMediaText(url)
        .then((text) => {
          if (!cancelled) setRows(parseCsv(text).slice(0, 200));
        })
        .catch((reason) => {
          if (!cancelled) setError(errorMessage(reason));
        });
    }
    return () => {
      cancelled = true;
    };
  }, [editor, kind, url]);

  if (!url) {
    return (
      <button
        type="button"
        className="bn-add-file-button"
        onMouseDown={(event) => event.preventDefault()}
        onClick={() => editor.getExtension(FilePanelExtension)?.showMenu(block.id)}
      >
        {iconFor(kind)}
        Add {kind.toUpperCase()}
      </button>
    );
  }

  return (
    <div className="media-embed">
      {kind === "pdf" && src && <iframe className="media-frame" src={src} title={name} />}
      {kind === "csv" && rows && <CsvTable rows={rows} />}
      {(kind === "docx" || kind === "xlsx") && (
        <div className="media-card">
          {iconFor(kind)}
          <span>{name}</span>
          <button type="button" onClick={() => void openMedia(url)}>
            Open
          </button>
        </div>
      )}
      {kind !== "docx" && kind !== "xlsx" && (
        <button type="button" className="media-open" onClick={() => void openMedia(url)}>
          Open {name}
        </button>
      )}
      {error && <p className="muted">{error}</p>}
    </div>
  );
}

function CsvTable({ rows }: { rows: string[][] }) {
  const [header, ...body] = rows;
  if (!header) return <p className="muted">Empty CSV.</p>;
  return (
    <div className="media-table-wrap">
      <table className="media-table">
        <thead>
          <tr>
            {header.slice(0, 30).map((cell, index) => (
              <th key={index}>{cell}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {body.map((row, rowIndex) => (
            <tr key={rowIndex}>
              {header.slice(0, 30).map((_, index) => (
                <td key={index}>{row[index] ?? ""}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function iconFor(kind: "pdf" | "csv" | "docx" | "xlsx") {
  if (kind === "pdf") return <FilePdf size={18} />;
  if (kind === "csv") return <FileCsv size={18} />;
  if (kind === "docx") return <FileDoc size={18} />;
  return <FileXls size={18} />;
}

function parseCsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  const source = text.replace(/^\uFEFF/, "");
  for (let index = 0; index < source.length; index += 1) {
    const char = source[index];
    if (quoted) {
      if (char === '"') {
        if (source[index + 1] === '"') {
          cell += '"';
          index += 1;
        } else {
          quoted = false;
        }
      } else {
        cell += char;
      }
      continue;
    }
    if (char === '"') {
      quoted = true;
    } else if (char === ",") {
      row.push(cell);
      cell = "";
    } else if (char === "\n") {
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
    } else if (char !== "\r") {
      cell += char;
    }
  }
  if (cell || row.length) {
    row.push(cell);
    rows.push(row);
  }
  return rows.filter((item) => item.some((value) => value.trim()));
}

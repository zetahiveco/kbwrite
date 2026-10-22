export type MediaKind = "image" | "video" | "audio" | "pdf" | "csv" | "docx" | "xlsx";

const KINDS: Record<string, MediaKind> = {
  png: "image",
  jpg: "image",
  jpeg: "image",
  gif: "image",
  webp: "image",
  svg: "image",
  bmp: "image",
  avif: "image",
  heic: "image",
  mp4: "video",
  webm: "video",
  mov: "video",
  ogv: "video",
  m4v: "video",
  mkv: "video",
  mp3: "audio",
  wav: "audio",
  ogg: "audio",
  m4a: "audio",
  aac: "audio",
  flac: "audio",
  pdf: "pdf",
  csv: "csv",
  docx: "docx",
  xlsx: "xlsx",
};

export function mediaKind(url: string): MediaKind | null {
  const path = url.split(/[?#]/)[0];
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return KINDS[ext] ?? null;
}

export function fileNameFromUrl(url: string): string {
  const path = url.split(/[?#]/)[0];
  const name = path.split("/").pop() || "file";
  try {
    return decodeURIComponent(name);
  } catch {
    return name;
  }
}

export function displayUrl(stored: string): string {
  if (/^(https?:|blob:|data:|asset:|vault:)/i.test(stored)) return stored;
  const relative = stored.replace(/^kbwrite-media:\/\/\//, "").replace(/^\/+/, "");
  const encoded = relative.split("/").map(encodeURIComponent).join("/");
  const windows = /Win/i.test(navigator.platform || "") || /Windows/i.test(navigator.userAgent || "");
  return windows ? `http://vault.localhost/${encoded}` : `vault://localhost/${encoded}`;
}

export async function fileToBase64(file: File): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  let binary = "";
  const size = 0x8000;
  for (let index = 0; index < bytes.length; index += size) {
    binary += String.fromCharCode(...bytes.subarray(index, index + size));
  }
  return btoa(binary);
}

export function restoreMediaMarkdown(markdown: string): string {
  return markdown.replace(
    /(^|[^!])\[([^\]]*)\]\((kbwrite-media:\/\/\/[^)\s]+)\)/g,
    (full, prefix: string, label: string, url: string) => {
      const kind = mediaKind(url);
      if (kind !== "pdf" && kind !== "csv" && kind !== "docx" && kind !== "xlsx") return full;
      const name = escapeAttr(label || fileNameFromUrl(url));
      return `${prefix}<figure><embed src="${escapeAttr(url)}" data-name="${name}" data-kbwrite="${kind}"></embed></figure>`;
    },
  );
}

function escapeAttr(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

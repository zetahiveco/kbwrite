// Roundtrip helper between vault markdown and the BlockNote editor.
//
// Fenced ` ```mermaid ` blocks and `$$ … $$` display math do not exist in
// BlockNote's markdown parser, so they travel as small placeholder `<div>`
// blocks while being parsed into the editor. Serialization needs no mirror
// transform: the block specs emit native `<math>` and code-fence HTML that
// BlockNote's markdown exporter turns back into `$$ … $$` and ` ```mermaid `.
// Inline `$ … $` math is handled inside the editor by the math plugin.

export function markdownToEditorMarkdown(markdown: string): string {
  const lines = markdown.split("\n");
  const out: string[] = [];
  let fence: string | null = null;
  let index = 0;
  while (index < lines.length) {
    const line = lines[index] ?? "";
    const marker = line.match(/^\s*(`{3,}|~{3,})([\w-]*)\s*$/);
    if (fence) {
      // A closing fence repeats the opening marker with no info string.
      if (marker && marker[1] === fence) fence = null;
      out.push(line);
      index += 1;
      continue;
    }
    if (marker) {
      fence = marker[1]!;
      if ((marker[2] ?? "").toLowerCase() === "mermaid") {
        const body: string[] = [];
        index += 1;
        while (
          index < lines.length &&
          !(lines[index] ?? "").match(new RegExp(`^\\s*${fence}\\s*$`))
        ) {
          body.push(lines[index] ?? "");
          index += 1;
        }
        index += 1; // closing fence (or past the end when unterminated)
        out.push(`<div data-kbwrite-mermaid="${encodeURIComponent(body.join("\n"))}"></div>`);
        fence = null;
        continue;
      }
      out.push(line);
      index += 1;
      continue;
    }
    // Single-line display math: `$$ x^2 $$`.
    const inline = line.match(/^\s*\$\$(.+)\$\$\s*$/);
    if (inline) {
      out.push(`<div data-kbwrite-math="${encodeURIComponent(inline[1]!.trim())}"></div>`);
      index += 1;
      continue;
    }
    // Multi-line display math:
    // $$
    // …
    // $$
    if (/^\s*\$\$\s*$/.test(line)) {
      const body: string[] = [];
      index += 1;
      while (index < lines.length && !/^\s*\$\$\s*$/.test(lines[index] ?? "")) {
        body.push(lines[index] ?? "");
        index += 1;
      }
      if (index < lines.length) {
        index += 1; // closing $$
        out.push(`<div data-kbwrite-math="${encodeURIComponent(body.join("\n"))}"></div>`);
        continue;
      }
      // Unterminated $$ — leave the lines as they were.
      out.push(line, ...body);
      continue;
    }
    out.push(line);
    index += 1;
  }
  return out.join("\n");
}

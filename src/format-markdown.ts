import * as prettier from "prettier/standalone";
import markdownPlugin from "prettier/plugins/markdown";

// Formats markdown with Prettier. Falls back to the original text when
// Prettier cannot parse it, so a broken note never blocks saving.
export async function formatMarkdown(markdown: string): Promise<string> {
  if (!markdown.trim()) return markdown;
  try {
    return await prettier.format(markdown, {
      parser: "markdown",
      plugins: [markdownPlugin],
      proseWrap: "preserve",
    });
  } catch {
    return markdown;
  }
}

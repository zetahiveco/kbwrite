// Headless roundtrip test for the new editor blocks (math, mermaid, code).
import { ServerBlockNoteEditor } from "@blocknote/server-util";
import { markdownToEditorMarkdown } from "../src/markdown-pipes";
import { createEditorSchema } from "../src/plugin-blocks";

const SAMPLE = [
  "# Note",
  "",
  "Some text with $x^2 + y^2$ inline math.",
  "",
  "$$",
  "\\int_0^1 x \\, dx = \\frac{1}{2}",
  "$$",
  "",
  "```mermaid",
  "flowchart TD",
  "  A[Start] --> B[End]",
  "```",
  "",
  "```ts",
  "const a: number = 1;",
  "```",
  "",
  "```md",
  "$$",
  "not math",
  "$$",
  "```",
  "",
  "- a [[Wikilink]] item",
  "",
  "Costs $5 and $10 total.",
  "",
].join("\n");

// The lossy exporter writes bullets as `*`; that is pre-existing BlockNote
// behavior, so compare against the canonical form of the sample.
const EXPECTED = SAMPLE
  .replace("- a [[Wikilink]] item", "* a [[Wikilink]] item");

async function main() {
  const editor = new ServerBlockNoteEditor({
    schema: createEditorSchema([]) as any,
  });
  const editorMd = markdownToEditorMarkdown(SAMPLE);
  const blocks = await editor.tryParseMarkdownToBlocks(editorMd);
  console.log(
    "--- parsed block types ---",
    JSON.stringify(blocks.map((b: any) => b.type)),
  );
  const serialized = await editor.blocksToMarkdownLossy(blocks as any);
  console.log("--- serialized ---\n" + serialized);
  console.log("--- roundtrip equal:", serialized === EXPECTED);
  if (serialized !== EXPECTED) {
    for (let i = 0; i < Math.max(serialized.length, EXPECTED.length); i++) {
      if (serialized[i] !== EXPECTED[i]) {
        console.log("first diff at", i, JSON.stringify(EXPECTED.slice(i - 30, i + 30)), "vs", JSON.stringify(serialized.slice(i - 30, i + 30)));
        break;
      }
    }
  }
  await (editor as any).close?.();
  process.exit(0);
}

void main();

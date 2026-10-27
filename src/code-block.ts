import {
  createBlockSpec,
  createCodeBlock,
  createCodeBlockConfig,
  createPreCode,
  parsePreCode,
  parsePreCodeContent,
} from "@blocknote/core";

// Languages offered in the code block picker. Keys are Shiki language ids.
// Fences with a language outside this list still render (without a picker
// entry) instead of crashing the editor.
export const CODE_LANGUAGES: Record<string, { name: string; aliases?: string[] }> = {
  text: { name: "Text", aliases: ["plain", "txt", "none", "plaintext"] },
  bash: { name: "Bash", aliases: ["sh", "shell", "zsh"] },
  c: { name: "C" },
  cpp: { name: "C++", aliases: ["c++"] },
  csharp: { name: "C#", aliases: ["c#", "cs"] },
  css: { name: "CSS" },
  diff: { name: "Diff" },
  dockerfile: { name: "Dockerfile", aliases: ["docker"] },
  go: { name: "Go", aliases: ["golang"] },
  html: { name: "HTML", aliases: ["xml", "svg"] },
  java: { name: "Java" },
  javascript: { name: "JavaScript", aliases: ["js", "jsx", "mjs", "node"] },
  json: { name: "JSON" },
  kotlin: { name: "Kotlin", aliases: ["kt"] },
  lua: { name: "Lua" },
  makefile: { name: "Makefile", aliases: ["make"] },
  markdown: { name: "Markdown", aliases: ["md"] },
  python: { name: "Python", aliases: ["py"] },
  ruby: { name: "Ruby", aliases: ["rb"] },
  rust: { name: "Rust", aliases: ["rs"] },
  scala: { name: "Scala" },
  shellscript: { name: "Shell" },
  sql: { name: "SQL" },
  swift: { name: "Swift" },
  toml: { name: "TOML" },
  typescript: { name: "TypeScript", aliases: ["ts", "tsx"] },
  yaml: { name: "YAML", aliases: ["yml"] },
};

/**
 * A replacement for the default code block that knows about a curated language
 * list (so the picker works) and falls back to plain rendering for fence
 * languages outside the list, instead of throwing.
 */
export const codeBlockSpec = createBlockSpec(
  createCodeBlockConfig({ supportedLanguages: CODE_LANGUAGES }),
  () => ({
    meta: {
      code: true,
      defining: true,
      isolating: false,
      highlight: (block: { props: { language: string } }) => block.props.language,
    },
    parse: parsePreCode,
    parseContent: (options: { el: HTMLElement; schema: any }) =>
      parsePreCodeContent(options, "codeBlock"),
    render: (block: { props: { language: string } }, editor: any) => {
      const language = block.props.language;
      if (language && !(language in CODE_LANGUAGES)) {
        return createCodeBlock(block as any, editor);
      }
      return createCodeBlock(block as any, editor, {
        selectedLanguage: language,
        supportedLanguages: CODE_LANGUAGES,
      });
    },
    toExternalHTML: (block: { props: { language: string } }) => createPreCode(block as any),
  }),
);

// Lazily created Shiki highlighter shared by every editor instance. Only a few
// common languages are preloaded; the rest load on first use.
export const shikiHighlighter = import("shiki").then(({ createHighlighter }) =>
  createHighlighter({
    themes: ["github-light", "github-dark"],
    langs: ["javascript", "typescript", "python", "rust", "json", "bash"],
  }),
);

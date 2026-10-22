export type Settings = {
  vaultPath: string;
  darkMode: boolean;
};

export type McpClient = "claude" | "codex" | "opencode";

export type McpStatus = {
  running: boolean;
  port: number;
  url: string;
  claude: string;
  codex: string;
  opencode: string;
};

export type NoteMeta = {
  path: string;
  rel: string;
  title: string;
};

export type TreeNode = {
  name: string;
  path: string;
  isDir: boolean;
  children: TreeNode[];
};

export type Snapshot = {
  settings: Settings;
  notes: NoteMeta[];
  tree: TreeNode[];
  walkErrors: string[];
  permissionDenied: boolean;
  vaultError: string | null;
};

export type Mutation = {
  path: string;
  snapshot: Snapshot;
};

export type GraphNode = {
  path: string;
  title: string;
};

export type GraphData = {
  nodes: GraphNode[];
  edges: [number, number][];
};

export type ViewMode = "editor" | "raw" | "graph";

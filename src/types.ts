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

export type PluginCommand = {
  id: string;
  name: string;
};

export type PluginBlockProp = {
  id: string;
  label: string;
  default: string;
};

export type PluginBlock = {
  type: string;
  title: string;
  description: string;
  group: string;
  props: PluginBlockProp[];
};

export type PluginDialogField = {
  id: string;
  label: string;
  kind: "text" | "password" | "checkbox";
  value: string;
};

export type PluginDialogRequest = {
  id: number;
  title: string;
  message: string;
  confirm: string;
  cancel: string;
  fields: PluginDialogField[];
};

export type PluginInfo = {
  id: string;
  name: string;
  version: string;
  description: string;
  authors: string;
  enabled: boolean;
  loaded: boolean;
  error: string | null;
  commands: PluginCommand[];
  blocks: PluginBlock[];
};

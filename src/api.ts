import { invoke } from "@tauri-apps/api/core";
import type { GraphData, McpStatus, Mutation, PluginInfo, Settings, Snapshot } from "./types";

export function getSnapshot(): Promise<Snapshot> {
  return invoke("get_snapshot");
}

export function reloadVault(path: string): Promise<Snapshot> {
  return invoke("reload_vault", { path });
}

export function readNote(path: string): Promise<string> {
  return invoke("read_note", { path });
}

export function writeNote(path: string, content: string): Promise<void> {
  return invoke("write_note", { path, content });
}

export function createFile(dir: string, name: string): Promise<Mutation> {
  return invoke("create_file", { dir, name });
}

export function createFolder(dir: string, name: string): Promise<Mutation> {
  return invoke("create_folder", { dir, name });
}

export function renamePath(from: string, name: string): Promise<Mutation> {
  return invoke("rename_path", { from, name });
}

export function deletePath(path: string): Promise<Snapshot> {
  return invoke("delete_path", { path });
}

export function saveSettings(settings: Settings): Promise<Snapshot> {
  return invoke("save_settings", { settings });
}

export function mcpStatus(): Promise<McpStatus> {
  return invoke("mcp_status");
}

export function mcpStart(): Promise<McpStatus> {
  return invoke("mcp_start");
}

export function mcpStop(): Promise<McpStatus> {
  return invoke("mcp_stop");
}

export function graph(): Promise<GraphData> {
  return invoke("graph");
}

export function openPrivacySettings(): Promise<void> {
  return invoke("open_privacy_settings");
}

export function saveMedia(notePath: string, name: string, dataBase64: string): Promise<string> {
  return invoke("save_media", { notePath, name, dataBase64 });
}

export function readMediaText(url: string): Promise<string> {
  return invoke("read_media_text", { url });
}

export function openMedia(url: string): Promise<void> {
  return invoke("open_media", { url });
}

export function listPlugins(): Promise<PluginInfo[]> {
  return invoke("list_plugins");
}

export function installPluginFolder(path: string): Promise<PluginInfo[]> {
  return invoke("install_plugin_folder", { path });
}

export function installPluginGithub(url: string): Promise<PluginInfo[]> {
  return invoke("install_plugin_github", { url });
}

export function setPluginEnabled(id: string, enabled: boolean): Promise<PluginInfo[]> {
  return invoke("set_plugin_enabled", { id, enabled });
}

export function removePlugin(id: string): Promise<PluginInfo[]> {
  return invoke("remove_plugin", { id });
}

export function runPluginCommand(pluginId: string, commandId: string): Promise<void> {
  return invoke("run_plugin_command", { pluginId, commandId });
}

export function resolvePluginDialog(id: number, response: string): Promise<void> {
  return invoke("resolve_plugin_dialog", { id, response });
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error && error.message) return error.message;
  return "Something went wrong";
}

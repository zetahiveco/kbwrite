import { invoke } from "@tauri-apps/api/core";
import type { GraphData, Mutation, Settings, Snapshot } from "./types";

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

export function aiComplete(instruction: string, text: string): Promise<string> {
  return invoke("ai_complete", { instruction, text });
}

export function graph(): Promise<GraphData> {
  return invoke("graph");
}

export function openPrivacySettings(): Promise<void> {
  return invoke("open_privacy_settings");
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error && error.message) return error.message;
  return "Something went wrong";
}

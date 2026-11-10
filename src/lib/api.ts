import { invoke } from "@tauri-apps/api/core";
import type { MissingReport, Resource, Space } from "./types";

export function listSpaces(): Promise<Space[]> {
  return invoke<Space[]>("list_spaces");
}

export function createSpace(name: string): Promise<Space> {
  return invoke<Space>("create_space", { name });
}

export function deleteSpace(spaceId: number): Promise<void> {
  return invoke<void>("delete_space", { spaceId });
}

export function listResources(spaceId: number): Promise<Resource[]> {
  return invoke<Resource[]>("list_resources", { spaceId });
}

export function addResource(
  spaceId: number,
  path: string,
  title?: string,
): Promise<Resource> {
  return invoke<Resource>("add_resource", { spaceId, path, title: title ?? null });
}

export function searchResources(
  query: string,
  spaceId?: number | null,
  limit = 100,
): Promise<Resource[]> {
  return invoke<Resource[]>("search_resources", {
    query,
    spaceId: spaceId ?? null,
    limit,
  });
}

export function updateResource(
  id: number,
  title: string,
  note: string,
): Promise<Resource> {
  return invoke<Resource>("update_resource", { id, title, note });
}

export function deleteResource(id: number): Promise<void> {
  return invoke<void>("delete_resource", { id });
}

export function checkMissingFiles(
  spaceId?: number | null,
): Promise<MissingReport> {
  return invoke<MissingReport>("check_missing_files", {
    spaceId: spaceId ?? null,
  });
}

export function revealInExplorer(path: string): Promise<void> {
  return invoke<void>("reveal_in_explorer", { path });
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}

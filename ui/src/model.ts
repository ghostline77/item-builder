export interface Recolour {
  region: string;
  colour: string;
}
export interface Project {
  format: "ghostline-item-project";
  version: 1;
  id: string;
  title: string;
  catalogue: string;
  item: string;
  mode: "variant" | "replace";
  record: string;
  damage_multiplier: number;
  magazine_bonus: number;
  reload_time_bonus: number;
  rarity: string | null;
  iconic: boolean | null;
  reload: "original" | "reloadable" | "disposable";
  appearance: string | null;
  recolours: Recolour[];
}
export interface Look {
  id: string;
  appearance: string;
  resolution: string;
  preview_state: string;
  media?: { view: string; path: string }[];
}
export interface Detail {
  item: {
    id: string;
    label: string;
    native_name: string;
    item_type: string;
    properties: unknown;
  };
  looks: Look[];
  reload_available: boolean;
}
export interface Bootstrap {
  catalogue: string;
  weapons: number;
  kinds: string[];
  sources: { game_version: string };
  browser_preview: boolean;
}
export interface SearchResult {
  items: {
    id: string;
    label: string;
    record: string;
    kind: string;
    looks: number;
  }[];
  total: number;
}
export interface Review {
  target: string;
  yaml: string;
  changes: { field: string; before: unknown; after: unknown; reason: string }[];
  warnings: string[];
  spawn_command: string | null;
}
export interface Region {
  id: string;
  label: string;
  colour: string;
}
export interface Saved {
  project: Project;
  revision: number | null;
  saved?: string;
}
export interface ColourLayer {
  node: string;
  material: string;
  colours: [number, number, number][];
  regions: string[];
  columns: number;
  png: number[];
}
export async function request(
  path: string,
  body?: unknown,
  method = "POST",
  signal?: AbortSignal,
): Promise<Response> {
  const response = await fetch(path, {
    ...(body === undefined
      ? {}
      : {
          method,
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(body),
        }),
    ...(signal ? { signal } : {}),
  });
  if (!response.ok) {
    const error = (await response
      .json()
      .catch(() => ({ error: response.statusText }))) as { error?: string };
    throw new Error(error.error || response.statusText);
  }
  return response;
}
export async function json<T>(
  path: string,
  body?: unknown,
  method = "POST",
  signal?: AbortSignal,
): Promise<T> {
  return (await request(path, body, method, signal)).json() as Promise<T>;
}
export const message = (error: unknown) =>
  error instanceof Error ? error.message : String(error);
export const mediaUrl = (path: string) =>
  "/media/" + path.split("/").map(encodeURIComponent).join("/");
export const visualKey = (p: Project) =>
  JSON.stringify([
    p.catalogue,
    p.item,
    p.appearance,
    p.recolours.slice().sort((a, b) => a.region.localeCompare(b.region)),
  ]);
export function download(blob: Blob, name: string) {
  const url = URL.createObjectURL(blob),
    a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 3000);
}
export function parseProject(value: unknown): Project {
  const p = value as Partial<Project> | null;
  if (
    !p ||
    p.format !== "ghostline-item-project" ||
    p.version !== 1 ||
    typeof p.item !== "string" ||
    typeof p.id !== "string"
  )
    throw new Error("Unsupported item project.");
  return { ...p, recolours: p.recolours || [] } as Project;
}

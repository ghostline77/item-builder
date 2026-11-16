import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button, Input } from "@ghostline/ui-core";
import { Viewport, nativeMaterials } from "@ghostline/ui-core/viewport";
import type { ViewportHandle } from "@ghostline/ui-core/viewport";
import type { Object3D } from "three";
import type { ColourLayer, Detail, Project } from "./model";
import { json, mediaUrl, message, request } from "./model";
import { liveColours } from "./live-colours";
interface Scene {
  url: string;
  layers: ColourLayer[];
}
const scenes = new Map<string, Scene>();
export function Preview({
  project,
  detail,
  native,
  region,
  highlight,
  onError,
}: {
  project: Project;
  detail: Detail;
  native: boolean;
  region: string;
  highlight: boolean;
  onError: (message: string) => void;
}) {
  const key = JSON.stringify([
    project.catalogue,
    project.item,
    project.appearance,
  ]);
  const [scene, setScene] = useState<Scene | null>(null),
    [loading, setLoading] = useState(false),
    [error, setError] = useState(""),
    [view, setView] = useState<"hero" | "left" | "front">("hero"),
    [zoom, setZoom] = useState(1),
    [highlightUrl, setHighlightUrl] = useState("");
  const viewport = useRef<ViewportHandle>(null),
    current = useRef(project);
  current.current = project;
  const colours = useMemo(liveColours, []);
  useEffect(() => {
    colours.set(project.recolours);
  }, [project.recolours, colours]);
  useEffect(() => {
    if (!native) return;
    const abort = new AbortController();
    setError("");
    setLoading(true);
    setScene(null);
    const cached = scenes.get(key);
    if (cached) {
      setScene(cached);
      setLoading(false);
      return;
    }
    const p = { ...current.current, recolours: [] };
    void Promise.all([
      request("/api/scene", p, "POST", abort.signal).then((response) =>
        response.blob(),
      ),
      json<ColourLayer[]>("/api/scene-layers", p, "POST", abort.signal),
    ])
      .then(([blob, layers]) => {
        if (abort.signal.aborted) return;
        const result = { url: URL.createObjectURL(blob), layers };
        scenes.set(key, result);
        setScene(result);
        while (scenes.size > 4) {
          const oldest = scenes.keys().next().value!;
          URL.revokeObjectURL(scenes.get(oldest)!.url);
          scenes.delete(oldest);
        }
      })
      .catch((error) => {
        if (!abort.signal.aborted) setError(message(error));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [key, native]);
  const adapter = useCallback(
    (object: Object3D, invalidate: () => void) => {
      nativeMaterials(object);
      colours.prepare(scene?.layers || []);
      colours.set(current.current.recolours);
      return colours.adapt(object, invalidate);
    },
    [scene, colours],
  );
  const source = useMemo(
    () => (scene ? { kind: "gltf" as const, url: scene.url } : undefined),
    [scene],
  );
  useEffect(() => {
    setHighlightUrl("");
    if (!highlight || !region) return;
    const abort = new AbortController();
    let url = "";
    void request(
      `/api/highlight?region=${encodeURIComponent(region)}&view=${view}`,
      current.current,
      "POST",
      abort.signal,
    )
      .then((response) => response.blob())
      .then((blob) => {
        if (!abort.signal.aborted) {
          url = URL.createObjectURL(blob);
          setHighlightUrl(url);
        }
      })
      .catch((error) => {
        if (!abort.signal.aborted) onError(message(error));
      });
    return () => {
      abort.abort();
      if (url) URL.revokeObjectURL(url);
    };
  }, [highlight, region, view, key, onError]);
  const look =
      detail.looks.find((l) => l.id === project.appearance) ||
      detail.looks.find((l) => l.media?.length) ||
      detail.looks[0],
    image = look?.media?.find((m) => m.view === view) || look?.media?.[0];
  return (
    <>
      <div className="preview" id="preview">
        {native ? (
          <Viewport
            ref={viewport}
            label="Item 3D preview"
            lighting="material"
            {...(source ? { source } : {})}
            materialAdapter={adapter}
            toolbar={false}
            grid={false}
            preserveCamera
            onError={(error) => setError(error.message)}
          />
        ) : image ? (
          <img
            id="render"
            src={mediaUrl(image.path)}
            alt={`${detail.item.label}, ${view}`}
            style={{ transform: `scale(${zoom})` }}
          />
        ) : (
          <p>No published preview for this selection.</p>
        )}
        {highlightUrl && (
          <img
            id="render"
            className="layer-reference"
            src={highlightUrl}
            alt="Magenta shows selected layer coverage"
          />
        )}
        <div className="preview-caption">
          <span>
            {error ||
              (loading
                ? "Preparing interactive scene…"
                : highlight
                  ? "Selected layer mask"
                  : native
                    ? "Interactive 3D"
                    : look?.appearance)}
          </span>
          <span>
            {native ? "Orbit · pan · zoom locally" : "Original finish"}
          </span>
        </div>
      </div>
      <div className="preview-tools">
        <div className="views" role="group" aria-label="Preview views">
          {(["hero", "left", "front"] as const).map((name) => (
            <Button
              key={name}
              aria-pressed={view === name}
              onClick={() => {
                setView(name);
                viewport.current?.fit(name === "hero" ? "perspective" : name);
              }}
            >
              {name}
            </Button>
          ))}
        </div>
        <div className="zoom">
          <label htmlFor="zoom">Zoom</label>
          <Input
            id="zoom"
            type="range"
            min="1"
            max="3"
            step="0.05"
            value={zoom}
            onChange={(event) => {
              const next = Number(event.target.value);
              viewport.current?.zoom(zoom / next);
              setZoom(next);
            }}
          />
          <Button
            variant="ghost"
            onClick={() => {
              setZoom(1);
              viewport.current?.fit();
            }}
          >
            Fit
          </Button>
        </div>
      </div>
    </>
  );
}

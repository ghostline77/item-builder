import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent, RefObject } from "react";
import { AppShell, Button, Input, Select } from "@ghostline/ui-core";
import { createRoot } from "react-dom/client";
import { useEditor } from "./useEditor";
import type { Editor } from "./useEditor";
import { Preview } from "./Preview";
import {
  download,
  json,
  mediaUrl,
  message,
  parseProject,
  request,
  visualKey,
} from "./model";
import type { Project, Region, SearchResult, Saved } from "./model";
import "./style.css";

function Library({ editor }: { editor: Editor }) {
  const [query, setQuery] = useState("Slaught"),
    [kind, setKind] = useState(""),
    [result, setResult] = useState<SearchResult>({ items: [], total: 0 }),
    [busy, setBusy] = useState(false);
  const version = useRef(0);
  useEffect(() => {
    const abort = new AbortController(),
      token = ++version.current;
    setBusy(true);
    const timer = setTimeout(() => {
      void json<SearchResult>(
        `/api/search?q=${encodeURIComponent(query)}&kind=${encodeURIComponent(kind)}&offset=0`,
        undefined,
        "GET",
        abort.signal,
      )
        .then(setResult)
        .catch((error) => {
          if (!abort.signal.aborted) editor.setStatus(message(error));
        })
        .finally(() => {
          if (token === version.current) setBusy(false);
        });
    }, 180);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [query, kind, editor.setStatus]);
  async function more() {
    const token = version.current;
    setBusy(true);
    try {
      const next = await json<SearchResult>(
        `/api/search?q=${encodeURIComponent(query)}&kind=${encodeURIComponent(kind)}&offset=${result.items.length}`,
      );
      if (token === version.current)
        setResult((previous) => ({
          items: [...previous.items, ...next.items],
          total: next.total,
        }));
    } catch (error) {
      editor.setStatus(message(error));
    } finally {
      if (token === version.current) setBusy(false);
    }
  }
  return (
    <aside className="library" aria-label="Weapon library">
      <div className="panel-title">
        <h1>Library</h1>
        <span className="badge">{result.total} found</span>
      </div>
      <label className="search-label" htmlFor="search">
        Find a weapon
      </label>
      <Input
        id="search"
        type="search"
        placeholder="Name or record ID"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
      />
      <label className="search-label" htmlFor="kind">
        Weapon type
      </label>
      <Select
        id="kind"
        value={kind}
        onChange={(event) => setKind(event.target.value)}
      >
        <option value="">All weapon types</option>
        {editor.bootstrap?.kinds.map((value) => (
          <option key={value} value={value}>
            {value.replace("Wea_", "")}
          </option>
        ))}
      </Select>
      <div className="results" aria-busy={busy}>
        {result.items.map((row) => (
          <Button
            key={row.id}
            className="result"
            aria-pressed={editor.project?.item === row.id}
            onClick={() => void editor.select(row.id)}
          >
            <strong>{row.label}</strong>
            <small>{row.record}</small>
            <small>
              {row.kind.replace("Wea_", "")} · {row.looks} native appearances
            </small>
          </Button>
        ))}
        {!busy && !result.items.length && (
          <p className="muted">No matching weapons.</p>
        )}
      </div>
      {result.items.length < result.total && (
        <Button disabled={busy} onClick={() => void more()}>
          Load more
        </Button>
      )}
    </aside>
  );
}
function ReviewPanel({
  editor,
  reviewRef,
}: {
  editor: Editor;
  reviewRef: RefObject<HTMLDetailsElement | null>;
}) {
  const review = editor.review;
  return (
    <details ref={reviewRef} className="review">
      <summary>
        <span>Review changes</span>
        <span className="badge">
          {editor.reviewBusy
            ? "Checking…"
            : review
              ? `${review.changes.length} changes`
              : "Needs attention"}
        </span>
      </summary>
      <div className="review-body">
        <div className="panel-title">
          <h2>Export review</h2>
          <Button onClick={editor.refreshReview}>Refresh</Button>
        </div>
        <p className="muted">
          Gameplay edits do not change the visual preview.
        </p>
        <div aria-live="polite">
          {editor.reviewBusy
            ? "Checking edits…"
            : editor.reviewError ||
              `${review?.changes.length || 0} changes · ${review?.target || ""}`}
        </div>
        {editor.project?.catalogue !== editor.bootstrap?.catalogue && (
          <Button
            onClick={() =>
              void json<Project>("/api/rebase", editor.project)
                .then((p) => {
                  editor.edit(p);
                  editor.setStatus(
                    "Rebased onto the current catalogue. Review every change before export.",
                  );
                })
                .catch((error) => editor.setStatus(message(error)))
            }
          >
            Rebase onto current catalogue
          </Button>
        )}
        {!!review?.changes.length && (
          <table>
            <thead>
              <tr>
                <th>Field</th>
                <th>Original / baseline</th>
                <th>Your edit</th>
              </tr>
            </thead>
            <tbody>
              {review.changes.map((change, index) => (
                <tr key={index} title={change.reason}>
                  <td>{change.field}</td>
                  <td>
                    {typeof change.before === "string"
                      ? change.before
                      : JSON.stringify(change.before)}
                  </td>
                  <td>
                    {typeof change.after === "string"
                      ? change.after
                      : JSON.stringify(change.after)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <ul className="notes">
          {review?.warnings.map((warning) => (
            <li key={warning}>{warning}</li>
          ))}
        </ul>
        <details>
          <summary>Generated TweakXL YAML</summary>
          <pre>
            <code>{review?.yaml}</code>
          </pre>
        </details>
        <details>
          <summary>Native source properties</summary>
          <pre>
            <code>
              {JSON.stringify(editor.detail?.item.properties, null, 2)}
            </code>
          </pre>
        </details>
        {review?.spawn_command && (
          <div>
            <label htmlFor="spawn">Acquire the variant in CET</label>
            <div className="copy-row">
              <Input id="spawn" readOnly value={review.spawn_command} />
              <Button
                onClick={() =>
                  void navigator.clipboard
                    .writeText(review.spawn_command!)
                    .then(() => editor.setStatus("CET command copied."))
                    .catch((error) => editor.setStatus(message(error)))
                }
              >
                Copy
              </Button>
            </div>
          </div>
        )}
      </div>
    </details>
  );
}
function NumberField({
  id,
  label,
  value,
  onChange,
  min,
  max,
}: {
  id: string;
  label: string;
  value: number;
  onChange: (value: number) => void;
  min: number;
  max: number;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  return (
    <>
      <label htmlFor={id}>{label}</label>
      <Input
        id={id}
        type="number"
        min={min}
        max={max}
        step="any"
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          if (event.target.value && event.target.validity.valid)
            onChange(Number(event.target.value));
        }}
        onBlur={() => setText(String(value))}
      />
    </>
  );
}
type Panel = "appearance" | "performance" | "export";
function Inspector({
  editor,
  region,
  setRegion,
  highlight,
  setHighlight,
  showReview,
}: {
  editor: Editor;
  region: string;
  setRegion: (id: string) => void;
  highlight: boolean;
  setHighlight: (value: boolean) => void;
  showReview: () => void;
}) {
  const p = editor.project!,
    detail = editor.detail!;
  const [panel, setPanel] = useState<Panel>("appearance"),
    [regions, setRegions] = useState<Region[]>([]),
    [materialSource, setMaterialSource] = useState(""),
    [materialStatus, setMaterialStatus] = useState(""),
    [busy, setBusy] = useState(false),
    [hex, setHex] = useState(""),
    [inventory, setInventory] = useState<{ key: string; url: string } | null>(
      null,
    );
  const selected = regions.find((r) => r.id === region) || regions[0],
    edit = p.recolours.find((e) => e.region === selected?.id),
    colour = edit?.colour || selected?.colour || "#B5F0CE";
  const current = useRef(p);
  current.current = p;
  useEffect(() => {
    setHex(colour);
  }, [colour]);
  useEffect(() => {
    setRegions([]);
    setMaterialSource("");
    setMaterialStatus("");
    setHighlight(false);
  }, [p.item, p.appearance, setHighlight]);
  useEffect(
    () => () => {
      if (inventory) URL.revokeObjectURL(inventory.url);
    },
    [inventory],
  );
  async function loadMaterials() {
    setBusy(true);
    setMaterialStatus("Reading material layers…");
    let project = p;
    if (!project.appearance) {
      const look = detail.looks.find((l) => l.media?.length) || detail.looks[0];
      if (!look) {
        setMaterialStatus("This record has no appearance to recolour.");
        setBusy(false);
        return;
      }
      project = { ...p, appearance: look.id };
      editor.edit({ appearance: look.id });
    }
    const source = project.appearance;
    try {
      const next = await json<Region[]>("/api/materials", project);
      if (current.current.appearance !== source) return;
      setRegions(next);
      setMaterialSource(source || "");
      setRegion(
        next.find((r) => r.id === project.recolours[0]?.region)?.id ||
          next[0]?.id ||
          "",
      );
      setMaterialStatus(
        next.length
          ? `${next.length} editable material layers`
          : "No editable multilayer colours were found for this appearance.",
      );
    } catch (error) {
      setMaterialStatus(message(error));
    } finally {
      setBusy(false);
    }
  }
  function applyColour(value: string) {
    setHex(value);
    if (!/^#[0-9a-f]{6}$/i.test(value) || !selected) return;
    const upper = value.toUpperCase();
    if (edit?.colour === upper) return;
    setHighlight(false);
    editor.edit({
      recolours: [
        ...p.recolours.filter((e) => e.region !== selected.id),
        { region: selected.id, colour: upper },
      ],
    });
  }
  async function renderIcon() {
    setBusy(true);
    setMaterialStatus("Rendering the export inventory icon…");
    const key = visualKey(p);
    try {
      const blob = await (
        await request("/api/preview?view=inventory", p)
      ).blob();
      setInventory({ key, url: URL.createObjectURL(blob) });
      setMaterialStatus("Inventory icon ready. Colour edits update locally.");
    } catch (error) {
      setMaterialStatus(message(error));
    } finally {
      setBusy(false);
    }
  }
  function tabKeys(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const tabs: Panel[] = ["appearance", "performance", "export"];
    let next: number | undefined;
    if (event.key === "ArrowRight") next = (index + 1) % 3;
    if (event.key === "ArrowLeft") next = (index + 2) % 3;
    if (event.key === "Home") next = 0;
    if (event.key === "End") next = 2;
    if (next !== undefined) {
      event.preventDefault();
      const name = tabs[next]!;
      setPanel(name);
      document.getElementById(`tab-${name}`)?.focus();
    }
  }
  return (
    <aside className="settings" aria-label="Item inspector">
      <div className="inspector-heading">
        <h2>Item</h2>
      </div>
      <div className="tabs" role="tablist" aria-label="Edit item">
        {(["appearance", "performance", "export"] as const).map(
          (name, index) => (
            <Button
              key={name}
              id={`tab-${name}`}
              role="tab"
              aria-controls={`panel-${name}`}
              aria-selected={panel === name}
              tabIndex={panel === name ? 0 : -1}
              onClick={() => setPanel(name)}
              onKeyDown={(event) => tabKeys(event, index)}
            >
              {name[0]!.toUpperCase() + name.slice(1)}
            </Button>
          ),
        )}
      </div>
      <form onSubmit={(event) => event.preventDefault()}>
        <section
          id="panel-appearance"
          role="tabpanel"
          aria-labelledby="tab-appearance"
          hidden={panel !== "appearance"}
        >
          <h3>Choose a finish</h3>
          <label htmlFor="appearance">Starting appearance</label>
          <Select
            id="appearance"
            value={p.appearance || ""}
            onChange={(event) =>
              editor.edit({
                appearance: event.target.value || null,
                recolours: [],
              })
            }
          >
            <option value="">Keep original</option>
            {detail.looks.map((look) => (
              <option
                key={look.id}
                value={look.id}
                disabled={look.resolution !== "resolved"}
              >
                {look.appearance || "default"}
                {look.preview_state === "ready" ? "" : " · preview unavailable"}
              </option>
            ))}
          </Select>
          <details className="finish-browser">
            <summary>Browse original finishes</summary>
            <div
              className="appearance-grid"
              role="group"
              aria-label="Native appearance previews"
            >
              {detail.looks.map((look) => (
                <Button
                  key={look.id}
                  className="finish"
                  aria-label={`Use ${look.appearance || "default"} appearance`}
                  aria-pressed={p.appearance === look.id}
                  disabled={look.resolution !== "resolved"}
                  onClick={() =>
                    editor.edit({ appearance: look.id, recolours: [] })
                  }
                >
                  {look.media?.[0] ? (
                    <img
                      src={mediaUrl(
                        (
                          look.media.find((m) => m.view === "hero") ||
                          look.media[0]
                        ).path,
                      )}
                      alt=""
                      loading="lazy"
                    />
                  ) : (
                    <span className="unavailable">No preview</span>
                  )}
                  <span className="finish-label">
                    {look.appearance || "default"}
                  </span>
                </Button>
              ))}
            </div>
          </details>
          <div className="material-note">
            <h3>Custom colours</h3>
            <Button
              className="wide"
              disabled={busy}
              onClick={() => void loadMaterials()}
            >
              Edit custom colours
            </Button>
            <p className="help" role="status">
              {materialStatus}
            </p>
            {materialSource === p.appearance && selected && (
              <div>
                <label htmlFor="region">Material layer</label>
                <Select
                  id="region"
                  value={selected.id}
                  onChange={(event) => setRegion(event.target.value)}
                >
                  {regions.map((r) => (
                    <option key={r.id} value={r.id}>
                      {r.label}
                    </option>
                  ))}
                </Select>
                <Button
                  aria-pressed={highlight}
                  className="wide"
                  onClick={() => setHighlight(!highlight)}
                >
                  Highlight affected area
                </Button>
                <div className="colour-fields">
                  <div>
                    <label htmlFor="colour">Colour</label>
                    <Input
                      id="colour"
                      type="color"
                      value={colour}
                      onChange={(event) => applyColour(event.target.value)}
                    />
                  </div>
                  <div>
                    <label htmlFor="colour-hex">Hex colour</label>
                    <Input
                      id="colour-hex"
                      value={hex}
                      maxLength={7}
                      pattern="#[0-9a-fA-F]{6}"
                      spellCheck={false}
                      onChange={(event) => applyColour(event.target.value)}
                      onBlur={() => setHex(colour)}
                    />
                  </div>
                </div>
                <p className="help">
                  {p.recolours.length} custom layer
                  {p.recolours.length === 1 ? "" : "s"} · Colours update locally
                </p>
                <div className="colour-actions">
                  <Button
                    disabled={!edit}
                    onClick={() => {
                      setHighlight(false);
                      editor.edit({
                        recolours: p.recolours.filter(
                          (e) => e.region !== selected.id,
                        ),
                      });
                    }}
                  >
                    Reset layer
                  </Button>
                  <Button
                    variant="primary"
                    disabled={busy || !p.recolours.length}
                    onClick={() => void renderIcon()}
                  >
                    Render inventory icon
                  </Button>
                </div>
              </div>
            )}
          </div>
        </section>
        <section
          id="panel-performance"
          role="tabpanel"
          aria-labelledby="tab-performance"
          hidden={panel !== "performance"}
        >
          <h3>Performance</h3>
          <NumberField
            id="damage"
            label="Damage multiplier"
            value={p.damage_multiplier}
            min={0.01}
            max={100}
            onChange={(value) => editor.edit({ damage_multiplier: value })}
          />
          <p className="help">
            1 = unchanged. Final damage also depends on level and game bonuses.
          </p>
          <NumberField
            id="magazine"
            label="Magazine size bonus (%)"
            value={Math.round(p.magazine_bonus * 10000) / 100}
            min={-95}
            max={1000}
            onChange={(value) => editor.edit({ magazine_bonus: value / 100 })}
          />
          <NumberField
            id="reload-time"
            label="Reload time change (%)"
            value={Math.round(p.reload_time_bonus * 10000) / 100}
            min={-40}
            max={40}
            onChange={(value) =>
              editor.edit({ reload_time_bonus: value / 100 })
            }
          />
          <p className="help">Negative is faster. Range: −40% to +40%.</p>
          <label htmlFor="rarity">Rarity</label>
          <Select
            id="rarity"
            value={p.rarity || ""}
            onChange={(event) =>
              editor.edit({ rarity: event.target.value || null })
            }
          >
            <option value="">Keep original</option>
            {["Common", "Uncommon", "Rare", "Epic", "Legendary"].map(
              (value) => (
                <option key={value}>{value}</option>
              ),
            )}
          </Select>
          <label htmlFor="iconic">Iconic status</label>
          <Select
            id="iconic"
            value={p.iconic == null ? "" : String(p.iconic)}
            onChange={(event) =>
              editor.edit({
                iconic:
                  event.target.value === ""
                    ? null
                    : event.target.value === "true",
              })
            }
          >
            <option value="">Keep original</option>
            <option value="true">Iconic</option>
            <option value="false">Not iconic</option>
          </Select>
          <h3>Reload behaviour</h3>
          <label htmlFor="reload">Reload profile</label>
          <Select
            id="reload"
            value={p.reload}
            onChange={(event) =>
              editor.edit({ reload: event.target.value as Project["reload"] })
            }
          >
            <option value="original">Keep original</option>
            <option value="reloadable" disabled={!detail.reload_available}>
              Make disposable handgun reloadable
            </option>
            <option
              value="disposable"
              disabled={detail.item.item_type !== "Wea_Handgun"}
            >
              Discard handgun when empty
            </option>
          </Select>
          <p className="help">
            {detail.reload_available
              ? "Reloadable applies the linked Unity profile: reserve handgun ammo, state machine, four timing stats and discard tag. Native reload clips still need in-game verification."
              : "Keep original for normal weapons. Disposable behaviour is available for handguns."}
          </p>
        </section>
        <section
          id="panel-export"
          role="tabpanel"
          aria-labelledby="tab-export"
          hidden={panel !== "export"}
        >
          <h3>Your mod</h3>
          {inventory?.key === visualKey(p) && (
            <figure id="inventory-preview">
              <img
                src={inventory.url}
                alt="Generated colour-matched inventory icon"
              />
              <figcaption>Colour-matched inventory icon</figcaption>
            </figure>
          )}
          <label htmlFor="title">Project title</label>
          <Input
            id="title"
            value={p.title}
            maxLength={120}
            required
            onChange={(event) => editor.edit({ title: event.target.value })}
          />
          <label htmlFor="mode">Export mode</label>
          <Select
            id="mode"
            value={p.mode}
            onChange={(event) =>
              editor.edit({ mode: event.target.value as Project["mode"] })
            }
          >
            <option value="variant">Create a separate variant</option>
            <option value="replace">Modify the original record</option>
          </Select>
          <p className="help">
            {p.mode === "variant"
              ? "Creates a derived record. The original weapon stays available."
              : "Changes this native record wherever the game uses it."}
          </p>
          <label htmlFor="record">New record ID</label>
          <Input
            id="record"
            value={p.record}
            disabled={p.mode === "replace"}
            placeholder="Items.MyWeapon"
            maxLength={86}
            onChange={(event) => editor.edit({ record: event.target.value })}
          />
          <Button className="wide" onClick={showReview}>
            Review changes
          </Button>
          <div className="dependency">
            <h3>Install your mod</h3>
            <ol>
              <li>
                Install RED4ext + TweakXL, plus ArchiveXL for custom colours.
              </li>
              <li>Choose Export mod above.</li>
              <li>Extract the ZIP into your game folder.</li>
            </ol>
            <p>The app does not install or change game files.</p>
          </div>
          <Button
            variant="ghost"
            className="wide"
            onClick={() =>
              download(
                new Blob([JSON.stringify(p, null, 2)], {
                  type: "application/json",
                }),
                `${p.id}.item.json`,
              )
            }
          >
            Export editable project JSON
          </Button>
        </section>
      </form>
    </aside>
  );
}
function ItemBuilder() {
  const editor = useEditor(),
    [region, setRegion] = useState(""),
    [highlight, setHighlight] = useState(false),
    [exporting, setExporting] = useState(false),
    [savedProjects, setSavedProjects] = useState<
      { id: string; title: string; revision: number }[]
    >([]),
    [showProjects, setShowProjects] = useState(false);
  const file = useRef<HTMLInputElement>(null),
    reviewRef = useRef<HTMLDetailsElement>(null),
    dialog = useRef<HTMLDialogElement>(null),
    p = editor.project;
  const report = useCallback(
    (text: string) => editor.setStatus(text),
    [editor.setStatus],
  );
  useEffect(() => {
    if (showProjects) dialog.current?.showModal();
    else dialog.current?.close();
  }, [showProjects]);
  async function exportMod() {
    if (!p) return;
    setExporting(true);
    try {
      const blob = await (await request("/api/export", p)).blob();
      download(blob, `${p.id}.zip`);
      editor.setStatus(
        "Mod downloaded. Extract into the game folder with RED4ext and TweakXL installed.",
      );
    } catch (error) {
      editor.setStatus(message(error));
    } finally {
      setExporting(false);
    }
  }
  const showReview = () => {
    if (reviewRef.current) {
      reviewRef.current.open = true;
      reviewRef.current.scrollIntoView({ block: "start" });
      reviewRef.current.querySelector("summary")?.focus();
    }
  };
  return (
    <AppShell
      product="Item builder"
      project={
        <span className="muted">
          {editor.bootstrap
            ? `${editor.bootstrap.weapons.toLocaleString()} weapon records · game ${editor.bootstrap.sources.game_version}`
            : "Loading catalogue…"}
        </span>
      }
      actions={
        <>
          <Button
            variant="ghost"
            onClick={() =>
              void json<typeof savedProjects>("/api/projects")
                .then((rows) => {
                  setSavedProjects(rows);
                  setShowProjects(true);
                })
                .catch((error) => editor.setStatus(message(error)))
            }
          >
            Open project
          </Button>
          <Button
            variant="ghost"
            onClick={() => {
              if (editor.canLeave()) file.current?.click();
            }}
          >
            Import JSON
          </Button>
          <input
            ref={file}
            type="file"
            accept=".json,application/json"
            hidden
            onChange={(event) => {
              const input = event.target,
                selected = input.files?.[0];
              if (selected)
                void selected
                  .text()
                  .then((text) => editor.open(parseProject(JSON.parse(text))))
                  .then(() =>
                    editor.setStatus(
                      "Project imported. Review it before exporting.",
                    ),
                  )
                  .catch((error) => editor.setStatus(message(error)))
                  .finally(() => {
                    input.value = "";
                  });
            }}
          />
          <Button disabled={!p} onClick={() => void editor.save()}>
            Save project
          </Button>
          <Button
            variant="primary"
            disabled={!p || exporting || editor.reviewBusy || !editor.review}
            onClick={() => void exportMod()}
          >
            {exporting ? "Exporting…" : "Export mod"}
          </Button>
        </>
      }
    >
      <div id="status" role="status" aria-live="polite">
        {editor.status}
      </div>
      <div className="workspace">
        <Library editor={editor} />
        <main>
          {p && editor.detail ? (
            <section>
              <div className="item-heading">
                <div>
                  <p className="eyebrow">
                    {editor.detail.item.item_type.replace("Wea_", "")}
                  </p>
                  <h2>{editor.detail.item.label}</h2>
                  <code>{editor.detail.item.native_name}</code>
                </div>
                <div className="history">
                  <Button
                    aria-label="Undo edit"
                    disabled={!editor.undo.length}
                    onClick={editor.undoEdit}
                  >
                    Undo
                  </Button>
                  <Button
                    aria-label="Redo edit"
                    disabled={!editor.redo.length}
                    onClick={editor.redoEdit}
                  >
                    Redo
                  </Button>
                  <span className="badge">
                    {editor.dirty ? "Unsaved" : "Saved"}
                  </span>
                </div>
              </div>
              <Preview
                project={p}
                detail={editor.detail}
                native={editor.bootstrap?.browser_preview || false}
                region={region}
                highlight={highlight}
                onError={report}
              />
              <ReviewPanel editor={editor} reviewRef={reviewRef} />
            </section>
          ) : (
            <section>
              <h2>Choose a weapon</h2>
              <p>
                Select its appearance, adjust its performance and export a mod.
              </p>
            </section>
          )}
        </main>
        {p && editor.detail && (
          <Inspector
            key={p.id}
            editor={editor}
            region={region}
            setRegion={setRegion}
            highlight={highlight}
            setHighlight={setHighlight}
            showReview={showReview}
          />
        )}
      </div>
      <dialog ref={dialog} onClose={() => setShowProjects(false)}>
        <div className="panel-title">
          <h2>Saved projects</h2>
          <Button
            aria-label="Close projects"
            onClick={() => setShowProjects(false)}
          >
            Close
          </Button>
        </div>
        <div id="projects-list">
          {savedProjects.length
            ? savedProjects.map((row) => (
                <Button
                  key={row.id}
                  onClick={() => {
                    if (!editor.canLeave()) return;
                    void json<Saved>(
                      `/api/projects/${encodeURIComponent(row.id)}`,
                    )
                      .then((data) =>
                        editor.open(
                          parseProject(data.project),
                          data.revision,
                          JSON.stringify(data.project),
                        ),
                      )
                      .then(() => {
                        setShowProjects(false);
                        editor.setStatus("Project opened.");
                      })
                      .catch((error) => editor.setStatus(message(error)));
                  }}
                >
                  {row.title} · revision {row.revision}
                </Button>
              ))
            : "No saved projects yet."}
        </div>
      </dialog>
    </AppShell>
  );
}
const root = document.getElementById("root");
if (!root) throw new Error("Application root is missing");
document.body.classList.add("gl-root");
createRoot(root).render(<ItemBuilder />);

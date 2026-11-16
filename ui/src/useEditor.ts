import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { json, message, parseProject } from "./model";
import type { Bootstrap, Detail, Project, Review, Saved } from "./model";

interface EditorState {
  project: Project | null;
  detail: Detail | null;
  revision: number | null;
  saved: string;
  undo: Project[];
  redo: Project[];
}
type Action =
  | {
      type: "open";
      project: Project;
      detail: Detail;
      revision: number | null;
      saved: string;
    }
  | { type: "edit"; changes: Partial<Project> }
  | { type: "undo" | "redo" }
  | { type: "saved"; id: string; revision: number; content: string };
function reduce(state: EditorState, action: Action): EditorState {
  if (action.type === "open") return { ...action, undo: [], redo: [] };
  if (action.type === "saved")
    return state.project?.id === action.id
      ? { ...state, revision: action.revision, saved: action.content }
      : state;
  if (!state.project) return state;
  if (action.type === "edit")
    return {
      ...state,
      project: { ...state.project, ...action.changes },
      undo: [...state.undo.slice(-99), state.project],
      redo: [],
    };
  const history = action.type === "undo" ? state.undo : state.redo,
    next = history.at(-1);
  if (!next) return state;
  return {
    ...state,
    project: next,
    undo:
      action.type === "undo"
        ? state.undo.slice(0, -1)
        : [...state.undo, state.project],
    redo:
      action.type === "redo"
        ? state.redo.slice(0, -1)
        : [...state.redo, state.project],
  };
}
export function useEditor() {
  const [state, dispatch] = useReducer(reduce, {
    project: null,
    detail: null,
    revision: null,
    saved: "",
    undo: [],
    redo: [],
  });
  const itemToken = useRef(0);
  const [bootstrap, setBootstrap] = useState<Bootstrap | null>(null),
    [status, setStatus] = useState("Loading catalogue…");
  const [review, setReview] = useState<Review | null>(null),
    [reviewError, setReviewError] = useState(""),
    [reviewBusy, setReviewBusy] = useState(false),
    [reviewTick, setReviewTick] = useState(0);
  const dirty =
    !!state.project && JSON.stringify(state.project) !== state.saved;
  const open = useCallback(
    async (p: Project, revision: number | null = null, saved = "") => {
      const token = ++itemToken.current;
      const detail = await json<Detail>(
        `/api/item?id=${encodeURIComponent(p.item)}`,
      );
      if (token !== itemToken.current) return;
      dispatch({ type: "open", project: p, detail, revision, saved });
    },
    [],
  );
  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const data = await json<Bootstrap>("/api/bootstrap");
        if (!active) return;
        setBootstrap(data);
        const recovery = localStorage.getItem("ghostline-item-builder-draft");
        if (recovery) {
          const draft = JSON.parse(recovery) as Saved;
          await open(
            parseProject(draft.project),
            draft.revision,
            draft.saved || "",
          );
          if (active) setStatus("Recovered your local draft.");
        } else setStatus("Choose a weapon to begin.");
      } catch (error) {
        if (active) setStatus(message(error));
      }
    })();
    return () => {
      active = false;
    };
  }, [open]);
  useEffect(() => {
    if (state.project)
      localStorage.setItem(
        "ghostline-item-builder-draft",
        JSON.stringify({
          project: state.project,
          revision: state.revision,
          saved: state.saved,
        }),
      );
  }, [state.project, state.revision, state.saved]);
  useEffect(() => {
    const leave = (event: BeforeUnloadEvent) => {
      if (dirty) {
        event.preventDefault();
        event.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", leave);
    return () => window.removeEventListener("beforeunload", leave);
  }, [dirty]);
  useEffect(() => {
    const p = state.project;
    if (!p) return;
    const abort = new AbortController();
    setReview(null);
    setReviewError("");
    setReviewBusy(true);
    const timer = setTimeout(() => {
      void json<Review>("/api/review", p, "POST", abort.signal)
        .then(setReview)
        .catch((error) => {
          if (!abort.signal.aborted) setReviewError(message(error));
        })
        .finally(() => {
          if (!abort.signal.aborted) setReviewBusy(false);
        });
    }, 240);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [state.project, reviewTick]);
  const edit = (changes: Partial<Project>) =>
    dispatch({ type: "edit", changes });
  const save = useCallback(async () => {
    if (!state.project) return;
    try {
      const content = JSON.stringify(state.project),
        saved = await json<{ revision: number }>(
          `/api/projects/${encodeURIComponent(state.project.id)}`,
          { project: state.project, revision: state.revision },
          "PUT",
        );
      dispatch({
        type: "saved",
        id: state.project.id,
        revision: saved.revision,
        content,
      });
      setStatus("Project saved locally.");
    } catch (error) {
      setStatus(message(error));
    }
  }, [state.project, state.revision]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "s") {
        event.preventDefault();
        void save();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [save]);
  const canLeave = () =>
    !dirty || confirm("Discard unsaved edits to this project?");
  async function select(id: string) {
    if (!canLeave()) return;
    const token = ++itemToken.current;
    try {
      setStatus("Loading weapon…");
      const [detail, p] = await Promise.all([
        json<Detail>(`/api/item?id=${encodeURIComponent(id)}`),
        json<Project>("/api/new", {
          item: id,
          id: `weapon_${Date.now().toString(36)}`,
        }),
      ]);
      if (token !== itemToken.current) return;
      p.title = `${detail.item.label} mod`;
      dispatch({ type: "open", project: p, detail, revision: null, saved: "" });
      setStatus(
        "New project. Edits stay local until you download and install the mod.",
      );
    } catch (error) {
      if (token === itemToken.current) setStatus(message(error));
    }
  }
  return {
    ...state,
    bootstrap,
    status,
    setStatus,
    dirty,
    review,
    reviewBusy,
    reviewError,
    edit,
    open,
    select,
    save,
    canLeave,
    undoEdit: () => dispatch({ type: "undo" }),
    redoEdit: () => dispatch({ type: "redo" }),
    refreshReview: () => setReviewTick((t) => t + 1),
  };
}
export type Editor = ReturnType<typeof useEditor>;

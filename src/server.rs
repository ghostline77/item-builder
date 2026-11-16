use crate::{
    catalogue::{Catalogue, relative, value},
    export,
    project::{Project, identifier},
};
use anyhow::Result;
use axum::{
    Json, Router,
    extract::Request,
    extract::{DefaultBodyLimit, Path as RoutePath, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
struct App {
    catalogue: Arc<Catalogue>,
    projects: Arc<Mutex<PathBuf>>,
    native: Option<Arc<crate::recolour::Native>>,
}

struct Error(StatusCode, String);
impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Self(StatusCode::UNPROCESSABLE_ENTITY, e.to_string())
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
type Api<T> = std::result::Result<T, Error>;
fn require(condition: bool, message: &str) -> Api<()> {
    if condition {
        Ok(())
    } else {
        Err(Error(StatusCode::UNPROCESSABLE_ENTITY, message.into()))
    }
}

#[derive(Serialize, Deserialize)]
struct Saved {
    revision: u64,
    project: Project,
}
#[derive(Deserialize)]
struct Save {
    revision: Option<u64>,
    project: Project,
}

pub async fn serve(
    catalogue: PathBuf,
    projects: PathBuf,
    port: u16,
    native_paths: Option<(PathBuf, Option<PathBuf>)>,
) -> Result<()> {
    let catalogue = Arc::new(Catalogue::load(&catalogue)?);
    let native = native_paths
        .map(|(game, schema)| crate::recolour::Native::open(game, schema).map(Arc::new))
        .transpose()?;
    fs::create_dir_all(&projects)?;
    let projects = Arc::new(Mutex::new(projects.canonicalize()?));
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let port = listener.local_addr()?.port();
    let app=Router::new()
        .route("/",get(||async {Html(include_str!("../ui/dist/index.html"))}))
        .route("/app.js",get(||async {([(header::CONTENT_TYPE,"text/javascript; charset=utf-8")],include_str!("../ui/dist/app.js"))}))
        .route("/style.css",get(||async {([(header::CONTENT_TYPE,"text/css; charset=utf-8")],include_str!("../ui/dist/style.css"))}))
        .route("/api/bootstrap",get(bootstrap))
        .route("/api/search",get(search))
        .route("/api/item",get(item))
        .route("/api/new",post(new_project))
        .route("/api/review",post(review))
        .route("/api/materials",post(materials))
        .route("/api/preview",post(preview))
        .route("/api/scene",post(browser_scene))
        .route("/api/scene-layers",post(browser_layers))
        .route("/api/highlight",post(highlight))
        .route("/api/rebase",post(rebase))
        .route("/api/export",post(package))
        .route("/api/projects",get(projects_list))
        .route("/api/projects/{id}",get(open_project).put(save_project))
        .route("/media/{*path}",get(media))
        .layer(DefaultBodyLimit::max(1024*1024))
        .layer(middleware::from_fn(move |request:Request,next:Next|async move {
            let valid_host=|s:&str|s==format!("127.0.0.1:{port}")||s==format!("localhost:{port}");
            let host=request.headers().get(header::HOST).and_then(|v|v.to_str().ok()).unwrap_or("");
            let valid_origin=request.headers().get(header::ORIGIN).is_none_or(|v|v.to_str().ok().and_then(|v|v.strip_prefix("http://")).is_some_and(valid_host));
            if !valid_host(host)||!valid_origin||request.headers().get("sec-fetch-site").is_some_and(|v|v=="cross-site") {
                return (StatusCode::FORBIDDEN,"Local builder origin required").into_response();
            }
            let mut response=next.run(request).await;
            response.headers_mut().insert("x-content-type-options","nosniff".parse().expect("header"));
            response.headers_mut().insert("content-security-policy","default-src 'self'; connect-src 'self' blob:; img-src 'self' data: blob:; object-src 'none'; frame-ancestors 'none'; base-uri 'none'".parse().expect("header"));
            response.headers_mut().insert(header::CACHE_CONTROL,"no-store".parse().expect("header"));
            response
        }))
        .with_state(App{catalogue,projects,native});
    println!("Ghostline Item Builder: http://127.0.0.1:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap(State(app): State<App>) -> Json<Value> {
    let kinds: std::collections::BTreeSet<_> = app
        .catalogue
        .items
        .values()
        .filter_map(|i| i["item_type"].as_str())
        .collect();
    Json(
        json!({"catalogue":app.catalogue.identity,"sources":app.catalogue.sources,"weapons":app.catalogue.items.len(),"kinds":kinds,"browser_preview":app.native.is_some(),"version":env!("CARGO_PKG_VERSION")}),
    )
}

#[derive(Deserialize)]
struct Search {
    #[serde(default)]
    q: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    offset: usize,
}
async fn search(State(app): State<App>, Query(q): Query<Search>) -> Json<Value> {
    Json(app.catalogue.search(&q.q, &q.kind, q.offset))
}
#[derive(Deserialize)]
struct Item {
    id: String,
}
async fn item(State(app): State<App>, Query(q): Query<Item>) -> Api<Json<Value>> {
    Ok(Json(tokio::task::spawn_blocking(move||->Result<Value>{
        let item=app.catalogue.item(&q.id)?;
        let looks=app.catalogue.selections(item)?;
        Ok(json!({"item":item,"looks":looks,"reload_available":item["item_type"]=="Wea_Handgun"&&value(item,"tags").as_array().is_some_and(|v|v.iter().any(|v|v=="DiscardOnEmpty"))}))
    }).await.map_err(anyhow::Error::from)??))
}
#[derive(Deserialize)]
struct New {
    id: String,
    item: String,
}
async fn new_project(State(app): State<App>, Json(input): Json<New>) -> Api<Json<Project>> {
    app.catalogue.item(&input.item)?;
    let p = Project::new(input.id, input.item, app.catalogue.identity.clone());
    p.validate()?;
    Ok(Json(p))
}
async fn review(State(app): State<App>, Json(p): Json<Project>) -> Api<Json<export::Review>> {
    Ok(Json(
        tokio::task::spawn_blocking(move || {
            if !p.recolours.is_empty() {
                app.native
                    .as_ref()
                    .ok_or_else(|| {
                        anyhow::anyhow!("start the server with --game to export custom colours")
                    })?
                    .validate(&app.catalogue, &p)?;
            }
            export::review(&app.catalogue, &p)
        })
        .await
        .map_err(anyhow::Error::from)??,
    ))
}
async fn rebase(State(app): State<App>, Json(mut p): Json<Project>) -> Api<Json<Project>> {
    p.catalogue = app.catalogue.identity.clone();
    export::review(&app.catalogue, &p)?;
    Ok(Json(p))
}
async fn package(State(app): State<App>, Json(p): Json<Project>) -> Api<Response> {
    let name = format!("attachment; filename=\"{}.zip\"", p.id);
    let bytes = tokio::task::spawn_blocking(move || {
        export::package(&app.catalogue, &p, app.native.as_deref())
    })
    .await
    .map_err(anyhow::Error::from)??;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/zip".parse().expect("header"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        name.parse().map_err(anyhow::Error::from)?,
    );
    Ok((headers, bytes).into_response())
}

async fn materials(
    State(app): State<App>,
    Json(p): Json<Project>,
) -> Api<Json<Vec<crate::recolour::Region>>> {
    Ok(Json(
        tokio::task::spawn_blocking(move || {
            export::review(&app.catalogue, &p)?;
            app.native
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("custom colours require --game"))?
                .regions(&app.catalogue, &p)
        })
        .await
        .map_err(anyhow::Error::from)??,
    ))
}
#[derive(Deserialize)]
struct PreviewView {
    view: Option<String>,
}
#[derive(Deserialize)]
struct HighlightQuery {
    region: String,
    view: Option<String>,
}
async fn highlight(
    State(app): State<App>,
    Query(q): Query<HighlightQuery>,
    Json(p): Json<Project>,
) -> Api<Response> {
    let view = match q.view.as_deref().unwrap_or("hero") {
        "hero" => ghostline_renderer::View::Hero,
        "left" => ghostline_renderer::View::Left,
        "front" => ghostline_renderer::View::Front,
        _ => {
            return Err(Error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "unknown preview view".into(),
            ));
        }
    };
    let image = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        export::review(&app.catalogue, &p)?;
        app.native
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("layer previews require game resources"))?
            .highlight(&app.catalogue, &p, &q.region, view)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(([(header::CONTENT_TYPE, "image/png")], image).into_response())
}
async fn browser_scene(State(app): State<App>, Json(p): Json<Project>) -> Api<Response> {
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        export::review(&app.catalogue, &p)?;
        let native = app
            .native
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("interactive preview requires --game"))?;
        Ok((*native.browser_scene(&app.catalogue, &p)?).clone())
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok((
        [
            (header::CONTENT_TYPE, "model/gltf-binary"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}

async fn browser_layers(State(app): State<App>, Json(p): Json<Project>) -> Api<Json<Value>> {
    Ok(Json(
        tokio::task::spawn_blocking(move || -> Result<Value> {
            export::review(&app.catalogue, &p)?;
            app.native
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("interactive preview requires --game"))?
                .browser_layers(&app.catalogue, &p)
        })
        .await
        .map_err(anyhow::Error::from)??,
    ))
}

async fn preview(
    State(app): State<App>,
    Query(q): Query<PreviewView>,
    Json(p): Json<Project>,
) -> Api<Response> {
    let view = q.view.unwrap_or_else(|| "hero".into());
    require(
        ["hero", "left", "front", "inventory"].contains(&view.as_str()),
        "unknown preview view",
    )?;
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        export::review(&app.catalogue, &p)?;
        let assets = app
            .native
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("custom previews require --game"))?
            .build(&app.catalogue, &p)?;
        if view == "inventory" {
            Ok(assets.inventory_icon.clone())
        } else {
            assets
                .previews
                .get(&view)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("preview absent"))
        }
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(([(header::CONTENT_TYPE, "image/png")], bytes).into_response())
}
async fn projects_list(State(app): State<App>) -> Api<Json<Value>> {
    let root = app
        .projects
        .lock()
        .map_err(|_| anyhow::anyhow!("project store poisoned"))?;
    let mut list = Vec::new();
    for file in fs::read_dir(&*root).map_err(anyhow::Error::from)? {
        let file = file.map_err(anyhow::Error::from)?;
        if file.path().extension().is_some_and(|s| s == "json") {
            let saved: Saved =
                serde_json::from_slice(&fs::read(file.path()).map_err(anyhow::Error::from)?)
                    .map_err(anyhow::Error::from)?;
            list.push(json!({"id":saved.project.id,"title":saved.project.title,"item":saved.project.item,"revision":saved.revision}));
        }
    }
    Ok(Json(json!(list)))
}
async fn open_project(
    State(app): State<App>,
    RoutePath(id): RoutePath<String>,
) -> Api<Json<Saved>> {
    require(identifier(&id), "invalid project ID")?;
    let root = app
        .projects
        .lock()
        .map_err(|_| anyhow::anyhow!("project store poisoned"))?;
    let path = root.join(format!("{id}.json"));
    if !path.exists() {
        return Err(Error(StatusCode::NOT_FOUND, "project not found".into()));
    }
    Ok(Json(
        serde_json::from_slice(&fs::read(path).map_err(anyhow::Error::from)?)
            .map_err(anyhow::Error::from)?,
    ))
}
async fn save_project(
    State(app): State<App>,
    RoutePath(id): RoutePath<String>,
    Json(input): Json<Save>,
) -> Api<Json<Saved>> {
    require(
        id == input.project.id,
        "project ID differs from destination",
    )?;
    export::review(&app.catalogue, &input.project)?;
    let root = app
        .projects
        .lock()
        .map_err(|_| anyhow::anyhow!("project store poisoned"))?;
    let path = root.join(format!("{id}.json"));
    let old = if path.exists() {
        Some(
            serde_json::from_slice::<Saved>(&fs::read(&path).map_err(anyhow::Error::from)?)
                .map_err(anyhow::Error::from)?
                .revision,
        )
    } else {
        None
    };
    if input.revision != old {
        return Err(Error(
            StatusCode::CONFLICT,
            "Project changed in another window. Reopen it before saving.".into(),
        ));
    }
    let saved = Saved {
        revision: old.unwrap_or(0) + 1,
        project: input.project,
    };
    let mut temp = tempfile::NamedTempFile::new_in(&*root).map_err(anyhow::Error::from)?;
    temp.write_all(&serde_json::to_vec_pretty(&saved).map_err(anyhow::Error::from)?)
        .map_err(anyhow::Error::from)?;
    temp.as_file().sync_all().map_err(anyhow::Error::from)?;
    temp.persist(path).map_err(anyhow::Error::from)?;
    Ok(Json(saved))
}
async fn media(State(app): State<App>, RoutePath(path): RoutePath<String>) -> Api<Response> {
    require(
        path.starts_with("media/images/") && path.ends_with(".png"),
        "only catalogue PNG previews can be requested",
    )?;
    let path = relative(&app.catalogue.root, &path)?;
    let bytes = tokio::task::spawn_blocking(move || fs::read(path))
        .await
        .map_err(anyhow::Error::from)?
        .map_err(anyhow::Error::from)?;
    Ok(([(header::CONTENT_TYPE, "image/png")], bytes).into_response())
}

use anyhow::{anyhow, Context, Result};
use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use rand::RngCore;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    env, fs,
    net::{IpAddr, SocketAddr},
    path::{Path as FsPath, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

const USER_ENV: &str = "BRAIN_ADMIN_USERNAME";
const PASS_ENV: &str = "BRAIN_ADMIN_PASSWORD";
const HOME_ENV: &str = "BRAIN_HOME";
const CREDENTIALS_FILE: &str = "credentials.json";
const SESSION_COOKIE: &str = "brain_session";
const MAX_BAD_LOGINS: i64 = 5;
const IMPORT_BODY_LIMIT_BYTES: usize = 50 * 1024 * 1024;

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Connection>>,
    secret: Arc<[u8; 32]>,
    app_home: PathBuf,
}

#[derive(Debug, Serialize)]
struct Project {
    name: String,
    idea_count: usize,
}

#[derive(Debug, Serialize)]
struct Idea {
    id: String,
    project: String,
    title: String,
    markdown: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct Settings {
    app_home: String,
    brain_dir: String,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct AdminCredentials {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct ProjectRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RenameProjectRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct DeleteProjectRequest {
    confirm: String,
}

#[derive(Debug, Deserialize)]
struct DeleteIdeasRequest {
    ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DeleteIdeasResponse {
    deleted: usize,
}

#[derive(Debug, Deserialize)]
struct SaveIdeaRequest {
    markdown: String,
}

#[derive(Debug, Deserialize)]
struct ImportMarkdownRequest {
    markdown: String,
}

#[derive(Debug, Serialize)]
struct ImportMarkdownResponse {
    imported: usize,
    ideas: Vec<Idea>,
}

#[derive(Debug, Deserialize)]
struct ImportIdeaRequest {
    from_project: String,
    id: String,
}

#[derive(Debug, Deserialize)]
struct SettingsRequest {
    brain_dir: String,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if !args.is_empty() {
        return cli(&args);
    }

    let app_home = app_home()?;
    fs::create_dir_all(&app_home)?;
    let db = Connection::open(app_home.join("brain.sqlite3"))?;
    migrate(&db)?;
    ensure_default_brain_dir(&db)?;

    let mut secret = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut secret);
    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        secret: Arc::new(secret),
        app_home,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/login", get(login_page))
        .route("/app.css", get(app_css))
        .route("/app.js", get(app_js))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/settings", get(get_settings).post(update_settings))
        .route("/api/projects", get(list_projects).post(create_project))
        .route(
            "/api/projects/:project",
            post(rename_project).delete(delete_project),
        )
        .route(
            "/api/projects/:project/ideas",
            get(list_project_ideas).post(save_idea).delete(delete_ideas),
        )
        .route("/api/projects/:project/import", post(import_idea))
        .route(
            "/api/projects/:project/import-markdown",
            post(import_markdown).layer(DefaultBodyLimit::max(IMPORT_BODY_LIMIT_BYTES)),
        )
        .route("/api/search", get(search_ideas))
        .route("/api/related", get(related_ideas))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = env::var("BRAIN_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let addr: SocketAddr = addr.parse().context("BRAIN_ADDR must be host:port")?;
    let listener = TcpListener::bind(addr).await?;
    println!("brain listening on http://{addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn cli(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("help") | Some("--help") | Some("-h") => print_help(),
        Some("credentials") if args.get(1).map(String::as_str) == Some("status") => {
            let app_home = app_home()?;
            let credentials_path = credentials_path(&app_home);
            println!(
                "{USER_ENV}: {}",
                env::var(USER_ENV).map(|_| "set").unwrap_or("missing")
            );
            println!(
                "{PASS_ENV}: {}",
                env::var(PASS_ENV).map(|_| "set").unwrap_or("missing")
            );
            println!(
                "credentials file: {} ({})",
                if credentials_path.exists() {
                    "set"
                } else {
                    "missing"
                },
                credentials_path.display()
            );
            println!(
                "login credentials: {}",
                credentials(&app_home).map(|_| "set").unwrap_or("missing")
            );
            Ok(())
        }
        Some("credentials") if args.get(1).map(String::as_str) == Some("set") => {
            let username = args.get(2).ok_or_else(|| anyhow!("missing username"))?;
            let password = args.get(3).ok_or_else(|| anyhow!("missing password"))?;
            set_credentials(username, password)
        }
        _ => {
            print_help()?;
            Ok(())
        }
    }
}

fn print_help() -> Result<()> {
    println!("brain");
    println!();
    println!("Commands:");
    println!("  brain                         Start the web server");
    println!("  brain credentials status      Show whether admin credentials are set");
    println!("  brain credentials set USER PASS");
    println!("                                Persist admin credentials in the app home");
    println!();
    println!("Environment:");
    println!("  {USER_ENV}       Admin login username");
    println!("  {PASS_ENV}       Admin login password");
    println!("  {HOME_ENV}                  App home, defaults to ~/.brain");
    println!("  BRAIN_ADDR                  Listen address, defaults to 127.0.0.1:8787");
    Ok(())
}

fn set_credentials(username: &str, password: &str) -> Result<()> {
    let app_home = app_home()?;
    write_credentials(&app_home, username, password)?;
    println!(
        "Credentials written to {}",
        credentials_path(&app_home).display()
    );
    Ok(())
}

async fn index(headers: HeaderMap, State(state): State<AppState>) -> Response {
    if !authenticated(&headers, &state) {
        return Redirect::to("/login").into_response();
    }
    Html(INDEX_HTML).into_response()
}

async fn login_page() -> Html<&'static str> {
    Html(LOGIN_HTML)
}

async fn app_css() -> Response {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], APP_CSS).into_response()
}

async fn app_js() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        APP_JS,
    )
        .into_response()
}

async fn login(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let ip = addr.ip();
    let db = state.db.lock().expect("db mutex poisoned");
    if let Err(err) = purge_old_attempts(&db) {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string());
    }
    match is_ip_banned(&db, ip) {
        Ok(true) => {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "too many failed logins from this IP",
            )
        }
        Ok(false) => {}
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }

    let Ok((username, password)) = credentials(&state.app_home) else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "admin credentials must be set with `brain credentials set USER PASS` before login",
        );
    };

    if req.username != username || req.password != password {
        if let Err(err) = record_bad_login(&db, ip) {
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string());
        }
        return json_error(StatusCode::UNAUTHORIZED, "invalid username or password");
    }

    let token = sign_session(&username, &state);
    let mut response = Json(serde_json::json!({ "ok": true })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800"
        ))
        .expect("valid cookie"),
    );
    response
}

async fn logout() -> Response {
    let mut response = Json(serde_json::json!({ "ok": true })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("brain_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
    );
    response
}

async fn get_settings(headers: HeaderMap, State(state): State<AppState>) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match brain_dir(&state) {
        Ok(brain_dir) => Json(Settings {
            app_home: state.app_home.display().to_string(),
            brain_dir: brain_dir.display().to_string(),
        })
        .into_response(),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn update_settings(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<SettingsRequest>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    let dir = PathBuf::from(req.brain_dir.trim());
    if dir.as_os_str().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "brain directory is required");
    }
    if let Err(err) = fs::create_dir_all(&dir) {
        return json_error(StatusCode::BAD_REQUEST, err.to_string());
    }
    let db = state.db.lock().expect("db mutex poisoned");
    if let Err(err) = set_setting(&db, "brain_dir", &dir.display().to_string()) {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string());
    }
    Json(Settings {
        app_home: state.app_home.display().to_string(),
        brain_dir: dir.display().to_string(),
    })
    .into_response()
}

async fn list_projects(headers: HeaderMap, State(state): State<AppState>) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match project_list(&state) {
        Ok(projects) => Json(projects).into_response(),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn create_project(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ProjectRequest>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match create_project_dir(&state, &req.name) {
        Ok(project) => (StatusCode::CREATED, Json(project)).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn rename_project(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(req): Json<RenameProjectRequest>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match rename_project_dir(&state, &project, &req.name) {
        Ok(project) => Json(project).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn delete_project(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(req): Json<DeleteProjectRequest>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    if req.confirm.trim() != "yes I want to delete that" {
        return json_error(
            StatusCode::BAD_REQUEST,
            "type exactly: yes I want to delete that",
        );
    }
    match delete_project_dir(&state, &project) {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn list_project_ideas(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match ideas_for_project(&state, &project, query.q.as_deref()) {
        Ok(ideas) => Json(ideas).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn save_idea(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(req): Json<SaveIdeaRequest>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match save_markdown_idea(&state, &project, &req.markdown) {
        Ok(idea) => (StatusCode::CREATED, Json(idea)).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn delete_ideas(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(req): Json<DeleteIdeasRequest>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match delete_project_ideas(&state, &project, &req.ids) {
        Ok(deleted) => Json(DeleteIdeasResponse { deleted }).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn import_idea(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(req): Json<ImportIdeaRequest>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match import_existing_idea(&state, &project, &req.from_project, &req.id) {
        Ok(idea) => (StatusCode::CREATED, Json(idea)).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn import_markdown(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(req): Json<ImportMarkdownRequest>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match import_markdown_ideas(&state, &project, &req.markdown) {
        Ok(ideas) => (
            StatusCode::CREATED,
            Json(ImportMarkdownResponse {
                imported: ideas.len(),
                ideas,
            }),
        )
            .into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn search_ideas(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match all_ideas(&state, query.q.as_deref()) {
        Ok(ideas) => Json(ideas).into_response(),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn related_ideas(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    if !authenticated(&headers, &state) {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    match related(&state, query.q.as_deref().unwrap_or_default()) {
        Ok(ideas) => Json(ideas).into_response(),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS login_attempts (
            id INTEGER PRIMARY KEY,
            ip TEXT NOT NULL,
            attempted_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_login_attempts_ip_time ON login_attempts(ip, attempted_at);
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn purge_old_attempts(db: &Connection) -> Result<()> {
    let cutoff = (Utc::now() - Duration::hours(24)).to_rfc3339();
    db.execute(
        "DELETE FROM login_attempts WHERE attempted_at < ?1",
        params![cutoff],
    )?;
    Ok(())
}

fn is_ip_banned(db: &Connection, ip: IpAddr) -> Result<bool> {
    let cutoff = (Utc::now() - Duration::hours(24)).to_rfc3339();
    let count: i64 = db.query_row(
        "SELECT COUNT(*) FROM login_attempts WHERE ip = ?1 AND attempted_at >= ?2",
        params![ip.to_string(), cutoff],
        |row| row.get(0),
    )?;
    Ok(count >= MAX_BAD_LOGINS)
}

fn record_bad_login(db: &Connection, ip: IpAddr) -> Result<()> {
    db.execute(
        "INSERT INTO login_attempts (ip, attempted_at) VALUES (?1, ?2)",
        params![ip.to_string(), Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

fn credentials(app_home: &FsPath) -> Result<(String, String)> {
    if let (Ok(username), Ok(password)) = (env::var(USER_ENV), env::var(PASS_ENV)) {
        return Ok((username, password));
    }
    let credentials = read_credentials(app_home)?;
    Ok((credentials.username, credentials.password))
}

fn credentials_path(app_home: &FsPath) -> PathBuf {
    app_home.join(CREDENTIALS_FILE)
}

fn read_credentials(app_home: &FsPath) -> Result<AdminCredentials> {
    let path = credentials_path(app_home);
    let content = fs::read_to_string(&path)
        .with_context(|| format!("could not read credentials from {}", path.display()))?;
    let credentials = serde_json::from_str::<AdminCredentials>(&content)
        .with_context(|| format!("credentials file is invalid: {}", path.display()))?;
    if credentials.username.is_empty() || credentials.password.is_empty() {
        return Err(anyhow!("credentials file has empty username or password"));
    }
    Ok(credentials)
}

fn write_credentials(app_home: &FsPath, username: &str, password: &str) -> Result<()> {
    if username.is_empty() || password.is_empty() {
        return Err(anyhow!("username and password are required"));
    }
    fs::create_dir_all(app_home)?;
    let path = credentials_path(app_home);
    let credentials = AdminCredentials {
        username: username.to_string(),
        password: password.to_string(),
    };
    fs::write(&path, serde_json::to_string_pretty(&credentials)?)?;
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn authenticated(headers: &HeaderMap, state: &AppState) -> bool {
    let Some(cookie_header) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(token) = cookie_header.split(';').find_map(|part| {
        part.trim()
            .strip_prefix(SESSION_COOKIE)
            .and_then(|v| v.strip_prefix('='))
    }) else {
        return false;
    };
    let Ok((username, _)) = credentials(&state.app_home) else {
        return false;
    };
    token == sign_session(&username, state)
}

fn sign_session(username: &str, state: &AppState) -> String {
    let mut mac = HmacSha256::new_from_slice(state.secret.as_ref()).expect("hmac accepts key");
    mac.update(username.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

fn app_home() -> Result<PathBuf> {
    if let Ok(home) = env::var(HOME_ENV) {
        return Ok(PathBuf::from(home));
    }
    dirs_next::home_dir()
        .map(|home| home.join(".brain"))
        .ok_or_else(|| anyhow!("could not find home directory"))
}

fn ensure_default_brain_dir(db: &Connection) -> Result<()> {
    if get_setting(db, "brain_dir")?.is_some() {
        return Ok(());
    }
    let dir = env::var(HOME_ENV).map(PathBuf::from).unwrap_or(app_home()?);
    fs::create_dir_all(&dir)?;
    set_setting(db, "brain_dir", &dir.display().to_string())
}

fn brain_dir(state: &AppState) -> Result<PathBuf> {
    let db = state.db.lock().expect("db mutex poisoned");
    let dir =
        get_setting(&db, "brain_dir")?.unwrap_or_else(|| state.app_home.display().to_string());
    Ok(PathBuf::from(dir))
}

fn get_setting(db: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = db.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
}

fn set_setting(db: &Connection, key: &str, value: &str) -> Result<()> {
    db.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn project_list(state: &AppState) -> Result<Vec<Project>> {
    prune_invalid_ideas(state)?;
    let dir = brain_dir(state)?;
    fs::create_dir_all(&dir)?;
    let mut projects = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let idea_count = fs::read_dir(entry.path())?
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
            .count();
        projects.push(Project { name, idea_count });
    }
    projects.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(projects)
}

fn create_project_dir(state: &AppState, name: &str) -> Result<Project> {
    let name = clean_project_name(name)?;
    let path = project_path(state, &name)?;
    if path.exists() {
        return Err(anyhow!("project already exists"));
    }
    fs::create_dir_all(&path)?;
    Ok(Project {
        name,
        idea_count: 0,
    })
}

fn rename_project_dir(state: &AppState, old: &str, new: &str) -> Result<Project> {
    let old = clean_project_name(old)?;
    let new = clean_project_name(new)?;
    let old_path = project_path(state, &old)?;
    let new_path = project_path(state, &new)?;
    if !old_path.exists() {
        return Err(anyhow!("project does not exist"));
    }
    if new_path.exists() {
        return Err(anyhow!("new project name already exists"));
    }
    fs::rename(old_path, &new_path)?;
    let idea_count = fs::read_dir(new_path)?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count();
    Ok(Project {
        name: new,
        idea_count,
    })
}

fn delete_project_dir(state: &AppState, project: &str) -> Result<()> {
    let project = clean_project_name(project)?;
    let path = project_path(state, &project)?;
    if !path.exists() {
        return Err(anyhow!("project does not exist"));
    }
    fs::remove_dir_all(path)?;
    Ok(())
}

fn project_path(state: &AppState, project: &str) -> Result<PathBuf> {
    Ok(brain_dir(state)?.join(clean_project_name(project)?))
}

fn clean_project_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("project name is required"));
    }
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(anyhow!("project name cannot contain path separators"));
    }
    Ok(name.to_string())
}

fn save_markdown_idea(state: &AppState, project: &str, markdown: &str) -> Result<Idea> {
    let project = clean_project_name(project)?;
    let project_dir = project_path(state, &project)?;
    if !project_dir.exists() {
        return Err(anyhow!("project does not exist"));
    }
    let title = validate_markdown(markdown)?;
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now();
    let stored = format!(
        "---\nid: {id}\ntitle: {}\ncreated_at: {}\n---\n\n{}",
        one_line(&title),
        created_at.to_rfc3339(),
        markdown.trim()
    );
    fs::write(
        project_dir.join(format!("{}-{}.md", slugify(&title), id)),
        stored,
    )?;
    Ok(Idea {
        id,
        project,
        title,
        markdown: markdown.trim().to_string(),
        created_at,
    })
}

fn import_existing_idea(
    state: &AppState,
    to_project: &str,
    from_project: &str,
    id: &str,
) -> Result<Idea> {
    let source = ideas_for_project(state, from_project, None)?
        .into_iter()
        .find(|idea| idea.id == id)
        .ok_or_else(|| anyhow!("source idea not found"))?;
    save_markdown_idea(state, to_project, &source.markdown)
}

fn delete_project_ideas(state: &AppState, project: &str, ids: &[String]) -> Result<usize> {
    let project = clean_project_name(project)?;
    let dir = project_path(state, &project)?;
    if !dir.exists() {
        return Err(anyhow!("project does not exist"));
    }
    if ids.is_empty() {
        return Err(anyhow!("select at least one idea to delete"));
    }
    let ids = ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let mut deleted = 0;
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let content = fs::read_to_string(entry.path())?;
        let idea = parse_stored_idea(&project, &content)?;
        if ids.contains(idea.id.as_str()) {
            fs::remove_file(entry.path())?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

fn ideas_for_project(state: &AppState, project: &str, query: Option<&str>) -> Result<Vec<Idea>> {
    prune_invalid_ideas(state)?;
    let project = clean_project_name(project)?;
    let dir = project_path(state, &project)?;
    if !dir.exists() {
        return Err(anyhow!("project does not exist"));
    }
    let mut ideas = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let content = fs::read_to_string(entry.path())?;
        let idea = parse_stored_idea(&project, &content)?;
        validate_markdown(&idea.markdown)?;
        ideas.push(idea);
    }
    filter_sort_ideas(ideas, query)
}

fn all_ideas(state: &AppState, query: Option<&str>) -> Result<Vec<Idea>> {
    let mut ideas = Vec::new();
    for project in project_list(state)? {
        ideas.extend(ideas_for_project(state, &project.name, None)?);
    }
    filter_sort_ideas(ideas, query)
}

fn related(state: &AppState, text: &str) -> Result<Vec<Idea>> {
    let words = keywords(text);
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let mut scored = all_ideas(state, None)?
        .into_iter()
        .filter_map(|idea| {
            let hay = format!("{} {}", idea.title, idea.markdown).to_lowercase();
            let score = words
                .iter()
                .filter(|word| hay.contains(word.as_str()))
                .count();
            (score > 0).then_some((score, idea))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.created_at.cmp(&a.1.created_at))
    });
    Ok(scored.into_iter().take(8).map(|(_, idea)| idea).collect())
}

fn filter_sort_ideas(mut ideas: Vec<Idea>, query: Option<&str>) -> Result<Vec<Idea>> {
    if let Some(query) = query.map(str::trim).filter(|q| !q.is_empty()) {
        let query = query.to_lowercase();
        ideas.retain(|idea| {
            idea.project.to_lowercase().contains(&query)
                || idea.id.to_lowercase().contains(&query)
                || idea.title.to_lowercase().contains(&query)
                || idea.markdown.to_lowercase().contains(&query)
        });
    }
    ideas.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.title.cmp(&b.title))
    });
    Ok(ideas)
}

fn parse_stored_idea(project: &str, content: &str) -> Result<Idea> {
    let rest = content
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow!("idea is missing metadata"))?;
    let (front, markdown) = rest
        .split_once("\n---\n\n")
        .ok_or_else(|| anyhow!("idea metadata is incomplete"))?;
    let mut id = None;
    let mut title = None;
    let mut created_at = None;
    for line in front.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "id" => id = Some(value.trim().to_string()),
            "title" => title = Some(value.trim().to_string()),
            "created_at" => created_at = Some(value.trim().parse::<DateTime<Utc>>()?),
            _ => {}
        }
    }
    Ok(Idea {
        id: id.ok_or_else(|| anyhow!("idea is missing id"))?,
        project: project.to_string(),
        title: title.ok_or_else(|| anyhow!("idea is missing title"))?,
        markdown: markdown.trim().to_string(),
        created_at: created_at.ok_or_else(|| anyhow!("idea is missing created_at"))?,
    })
}

fn import_markdown_ideas(state: &AppState, project: &str, markdown: &str) -> Result<Vec<Idea>> {
    let ideas = split_import_markdown(markdown)?;
    let mut saved = Vec::with_capacity(ideas.len());
    for idea in ideas {
        saved.push(save_markdown_idea(state, project, &idea)?);
    }
    Ok(saved)
}

fn split_import_markdown(markdown: &str) -> Result<Vec<String>> {
    let markdown = markdown.trim();
    if markdown.is_empty() {
        return Err(anyhow!("markdown is required"));
    }
    let import_lines = normalized_import_lines(markdown);
    if !import_lines
        .iter()
        .find(|(_, line)| !line.trim().is_empty() && line.trim() != "---")
        .map(|(_, line)| line.trim().starts_with("# "))
        .unwrap_or(false)
    {
        return Err(anyhow!(
            "markdown import must start with a single '# Title' header"
        ));
    }

    let mut ideas = Vec::new();
    let mut current = Vec::new();
    let mut current_has_body = false;
    for (idx, line) in import_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() && current.is_empty() {
            continue;
        }
        if trimmed == "---" {
            if current.is_empty() {
                continue;
            }
            if !current_has_body {
                return Err(anyhow!(
                    "line {} separates an idea before it has text",
                    idx + 1
                ));
            }
            ideas.push(current.join("\n").trim().to_string());
            current.clear();
            current_has_body = false;
            continue;
        }
        if trimmed.starts_with('#') {
            if !is_single_title_header(trimmed) {
                return Err(anyhow!(
                    "line {} must be a single '# Title' header",
                    idx + 1
                ));
            }
            if !current.is_empty() {
                if !current_has_body {
                    return Err(anyhow!(
                        "line {} starts a new idea before the previous idea has text",
                        idx + 1
                    ));
                }
                ideas.push(current.join("\n").trim().to_string());
                current.clear();
                current_has_body = false;
            }
        } else if !trimmed.is_empty() {
            validate_paragraph_line(trimmed, idx + 1)?;
            current_has_body = true;
        }
        current.push(line);
    }

    if current.is_empty() {
        if ideas.is_empty() {
            return Err(anyhow!("markdown import must include at least one idea"));
        }
        return Ok(ideas);
    }
    if !current_has_body {
        return Err(anyhow!("last idea must include text after its header"));
    }
    ideas.push(current.join("\n").trim().to_string());

    for idea in &ideas {
        validate_markdown(idea)?;
    }
    Ok(ideas)
}

fn normalized_import_lines(markdown: &str) -> Vec<(usize, String)> {
    markdown
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let trimmed = line.trim();
            if trimmed.starts_with("<!-- project:") && trimmed.ends_with("-->") {
                None
            } else {
                Some((idx, line.to_string()))
            }
        })
        .collect()
}

fn prune_invalid_ideas(state: &AppState) -> Result<usize> {
    let dir = brain_dir(state)?;
    fs::create_dir_all(&dir)?;
    let mut removed = 0;
    for project in fs::read_dir(&dir)? {
        let project = project?;
        if !project.file_type()?.is_dir() {
            continue;
        }
        for entry in fs::read_dir(project.path())? {
            let entry = entry?;
            if entry.path().extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let Ok(content) = fs::read_to_string(entry.path()) else {
                fs::remove_file(entry.path())?;
                removed += 1;
                continue;
            };
            let valid = parse_stored_idea(&project.file_name().to_string_lossy(), &content)
                .and_then(|idea| validate_markdown(&idea.markdown))
                .is_ok();
            if !valid {
                fs::remove_file(entry.path())?;
                removed += 1;
            }
        }
    }
    Ok(removed)
}

fn validate_markdown(markdown: &str) -> Result<String> {
    let markdown = markdown.trim();
    if markdown.is_empty() {
        return Err(anyhow!("markdown is required"));
    }
    let mut lines = markdown.lines();
    let first = lines.next().unwrap_or_default();
    if !first.starts_with("# ") {
        return Err(anyhow!(
            "markdown must start with a single '# Title' header"
        ));
    }
    let title = first.trim_start_matches("# ").trim();
    if title.is_empty() {
        return Err(anyhow!("first header must include a title"));
    }
    let mut has_body = false;
    for (idx, line) in markdown.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            if idx == 0 && is_single_title_header(trimmed) {
                continue;
            }
            return Err(anyhow!(
                "line {} is not allowed; an idea can only have one '# Title' header",
                idx + 1
            ));
        }
        validate_paragraph_line(trimmed, idx + 1)?;
        has_body = true;
    }
    if !has_body {
        return Err(anyhow!("idea must include text after its title"));
    }
    Ok(title.to_string())
}

fn is_single_title_header(line: &str) -> bool {
    line.starts_with("# ") && !line.starts_with("## ")
}

fn validate_paragraph_line(line: &str, line_number: usize) -> Result<()> {
    if line.starts_with(['-', '*', '>', '`', '|'])
        || line.chars().all(|c| c == '-')
        || line.chars().all(|c| c == '=')
    {
        return Err(anyhow!(
            "line {} is not a paragraph; only title headers and paragraphs are accepted",
            line_number
        ));
    }
    Ok(())
}

fn keywords(text: &str) -> Vec<String> {
    let stop = [
        "about", "after", "again", "also", "because", "before", "being", "could", "from", "have",
        "into", "just", "like", "that", "their", "there", "these", "this", "with", "would", "your",
    ];
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|word| word.len() > 3 && !stop.contains(&word.as_str()))
        .take(24)
        .collect()
}

fn slugify(value: &str) -> String {
    let slug = value
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "idea".to_string()
    } else {
        slug
    }
}

fn one_line(value: &str) -> String {
    value.replace('\n', " ").replace(':', " -")
}

fn json_error(status: StatusCode, error: impl Into<String>) -> Response {
    (
        status,
        Json(ApiError {
            error: error.into(),
        }),
    )
        .into_response()
}

const LOGIN_HTML: &str = include_str!("../static/login.html");
const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_CSS: &str = include_str!("../static/app.css");
const APP_JS: &str = include_str!("../static/app.js");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_voice_markdown() {
        let title =
            validate_markdown("# Server\n\nThe server stores ideas.\n\nMore text.").unwrap();
        assert_eq!(title, "Server");
    }

    #[test]
    fn rejects_missing_single_hash_title() {
        let err = validate_markdown("## Server\nText").unwrap_err();
        assert!(err.to_string().contains("single '# Title'"));
    }

    #[test]
    fn rejects_non_paragraph_blocks() {
        let err = validate_markdown("# Server\n- item").unwrap_err();
        assert!(err
            .to_string()
            .contains("only title headers and paragraphs"));
    }

    #[test]
    fn rejects_nested_headers_inside_an_idea() {
        let err = validate_markdown("# Server\n\nText.\n\n# API\n\nMore text.").unwrap_err();
        assert!(err.to_string().contains("only have one"));
    }

    #[test]
    fn rejects_title_without_body_text() {
        let err = validate_markdown("# Server").unwrap_err();
        assert!(err.to_string().contains("text after its title"));
    }

    #[test]
    fn splits_valid_import_markdown() {
        let ideas = split_import_markdown("# One\n\nFirst text.\n\n# Two\n\nSecond text.").unwrap();
        assert_eq!(ideas.len(), 2);
        assert_eq!(ideas[0], "# One\n\nFirst text.");
        assert_eq!(ideas[1], "# Two\n\nSecond text.");
    }

    #[test]
    fn imports_brain_export_markdown() {
        let export = "<!-- project: brain | created_at: 2026-06-13T15:00:00Z | id: one -->\n\n# One\n\nFirst text.\n\n---\n\n<!-- project: brain | created_at: 2026-06-13T15:01:00Z | id: two -->\n\n# Two\n\nSecond text.";
        let ideas = split_import_markdown(export).unwrap();
        assert_eq!(ideas.len(), 2);
        assert_eq!(ideas[0], "# One\n\nFirst text.");
        assert_eq!(ideas[1], "# Two\n\nSecond text.");
    }

    #[test]
    fn deletes_selected_ideas_by_id() {
        let temp = tempfile::tempdir().unwrap();
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        set_setting(&db, "brain_dir", temp.path().to_str().unwrap()).unwrap();
        let state = AppState {
            db: Arc::new(Mutex::new(db)),
            secret: Arc::new([0_u8; 32]),
            app_home: temp.path().join("app"),
        };
        create_project_dir(&state, "project").unwrap();
        let first = save_markdown_idea(&state, "project", "# One\n\nFirst text.").unwrap();
        let second = save_markdown_idea(&state, "project", "# Two\n\nSecond text.").unwrap();

        let deleted = delete_project_ideas(&state, "project", &[first.id]).unwrap();

        assert_eq!(deleted, 1);
        let remaining = ideas_for_project(&state, "project", None).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second.id);
    }

    #[test]
    fn rejects_import_with_adjacent_headers() {
        let err = split_import_markdown("# One\n\n# Two\n\nSecond text.").unwrap_err();
        assert!(err
            .to_string()
            .contains("before the previous idea has text"));
    }

    #[test]
    fn rejects_import_with_nested_header_levels() {
        let err = split_import_markdown("# One\n\nText.\n\n## Two\n\nSecond text.").unwrap_err();
        assert!(err.to_string().contains("single '# Title'"));
    }

    #[test]
    fn slugifies_titles() {
        assert_eq!(slugify("My Project: API"), "my-project-api");
    }

    #[test]
    fn sqlite_bans_after_bad_logins() {
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        for _ in 0..MAX_BAD_LOGINS {
            record_bad_login(&db, ip).unwrap();
        }
        assert!(is_ip_banned(&db, ip).unwrap());
    }

    #[test]
    fn credentials_round_trip_through_app_home() {
        let temp = tempfile::tempdir().unwrap();

        write_credentials(temp.path(), "admin", "change-this").unwrap();
        let credentials = read_credentials(temp.path()).unwrap();

        assert_eq!(credentials.username, "admin");
        assert_eq!(credentials.password, "change-this");
        assert!(credentials_path(temp.path()).exists());
    }
}

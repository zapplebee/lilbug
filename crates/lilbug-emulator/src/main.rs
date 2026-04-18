use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use font8x8::{BASIC_FONTS, UnicodeFonts};
use lilbug_core::{
    ApiError, CommandRequest, ConfigPatchRequest, DEFAULT_WIFI_URL, DISPLAY_SIZE, DeviceState,
    FaceExpression, InitRequest, InitResponse, MotorDirection, PersistedDeviceState, RenderMode,
    StartupMode, WINDOW_HEIGHT, sha256_fingerprint,
};
use minifb::{Key, Window, WindowOptions};
use png::Encoder;
use rcgen::generate_simple_self_signed;

const WINDOW_WIDTH: usize = DISPLAY_SIZE;

const COLOR_BG: u32 = 0x101218;
const COLOR_RING: u32 = 0x6FE3FF;
const COLOR_TEXT_DIM: u32 = 0x596170;
const COLOR_TEXT_ACTIVE: u32 = 0xF9F871;
const COLOR_WHITE: u32 = 0xF5F7FA;
const COLOR_FACE_NEUTRAL: u32 = 0x273449;
const COLOR_FACE_HAPPY: u32 = 0x2F5B43;
const COLOR_FACE_BLINK: u32 = 0x5B4630;
const COLOR_FACE_SURPRISED: u32 = 0x59406A;

#[derive(Parser, Debug, Clone)]
#[command(name = "lilbug-emulator", about = "Native rev1 lilbug emulator")]
struct Args {
    #[arg(long, default_value = "bootstrap")]
    mode: StartupMode,

    #[arg(long, default_value = "127.0.0.1:8443")]
    https_addr: String,

    #[arg(long)]
    wifi_base_url: Option<String>,

    #[arg(long, default_value = ".lilbug-emulator")]
    storage_dir: PathBuf,

    #[arg(long)]
    headless: bool,

    #[arg(long = "run-for-ms")]
    run_for_ms: Option<u64>,
}

#[derive(Clone)]
struct AppState {
    runtime: Arc<Mutex<RuntimeState>>,
}

struct RuntimeState {
    device: DeviceState,
    persisted: Option<PersistedDeviceState>,
    storage_dir: PathBuf,
    base_url: String,
}

impl RuntimeState {
    fn require_wifi_ready(&self) -> Result<(), ResponseError> {
        if self.device.mode != StartupMode::Wifi {
            return Err(ResponseError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "wrong_mode",
                "normal-operation routes are only available in wifi mode",
            ));
        }

        if !self.device.provisioned {
            return Err(ResponseError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "not_provisioned",
                "device is not provisioned",
            ));
        }

        Ok(())
    }
}

#[derive(Debug)]
struct ResponseError {
    status: StatusCode,
    body: ApiError,
}

impl ResponseError {
    fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            body: ApiError {
                code: code.into(),
                message: message.into(),
            },
        }
    }
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.body)).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let certs = load_or_create_certs(&args.storage_dir)?;
    let persisted = load_persisted_state(&args.storage_dir)?;

    if args.mode == StartupMode::Wifi && persisted.is_none() {
        bail!(
            "wifi mode requires an existing provisioned state in {}",
            persisted_path(&args.storage_dir).display()
        );
    }

    let base_url = args
        .wifi_base_url
        .clone()
        .unwrap_or_else(|| default_base_url(args.mode));
    let runtime = Arc::new(Mutex::new(RuntimeState {
        device: DeviceState::from_persisted(args.mode, persisted.as_ref()),
        persisted,
        storage_dir: args.storage_dir.clone(),
        base_url,
    }));
    let app_state = AppState { runtime };

    let addr: SocketAddr = args
        .https_addr
        .parse()
        .with_context(|| format!("invalid --https-addr {}", args.https_addr))?;
    let tls = RustlsConfig::from_pem(certs.cert_pem.into_bytes(), certs.key_pem.into_bytes())
        .await
        .context("failed to build rustls config")?;

    let server = tokio::spawn(run_server(addr, tls, app_state.clone()));

    let ui_result = if args.headless {
        run_headless(args.run_for_ms)
    } else {
        let state = app_state.clone();
        tokio::task::spawn_blocking(move || run_window(state, args.run_for_ms))
            .await
            .map_err(|err| anyhow!("window task failed: {err}"))?
    };

    server.abort();
    let _ = server.await;
    ui_result
}

async fn run_server(addr: SocketAddr, tls: RustlsConfig, state: AppState) -> Result<()> {
    let app = Router::new()
        .route("/v1/init", post(init_device))
        .route("/v1/state", get(get_state))
        .route("/v1/config", get(get_config).post(update_config))
        .route("/v1/cmd", post(run_command))
        .route("/v1/frame.png", get(get_frame))
        .with_state(state);

    axum_server::bind_rustls(addr, tls)
        .serve(app.into_make_service())
        .await
        .context("https server failed")
}

async fn init_device(
    State(state): State<AppState>,
    Json(request): Json<InitRequest>,
) -> Result<Json<InitResponse>, ResponseError> {
    let mut runtime = state.runtime.lock().expect("state lock poisoned");

    if runtime.device.mode != StartupMode::Bootstrap {
        return Err(ResponseError::new(
            StatusCode::NOT_FOUND,
            "init_unavailable",
            "/v1/init is only available in bootstrap mode",
        ));
    }

    if runtime.device.provisioned {
        return Err(ResponseError::new(
            StatusCode::CONFLICT,
            "already_provisioned",
            "device is already provisioned",
        ));
    }

    if request.nickname.trim().is_empty() {
        return Err(ResponseError::new(
            StatusCode::BAD_REQUEST,
            "invalid_init",
            "nickname must not be empty",
        ));
    }
    if request.api_key.trim().is_empty() {
        return Err(ResponseError::new(
            StatusCode::BAD_REQUEST,
            "invalid_init",
            "api_key must not be empty",
        ));
    }

    let cert_pem = fs::read_to_string(cert_path(&runtime.storage_dir))
        .map_err(|err| ResponseError::new(StatusCode::INTERNAL_SERVER_ERROR, "io_error", err.to_string()))?;
    let cert_fingerprint = sha256_fingerprint(cert_pem.as_bytes());
    let persisted = PersistedDeviceState {
        config: lilbug_core::DeviceConfig {
            nickname: request.nickname.clone(),
            wifi: lilbug_core::WifiConfig {
                ssid: request.wifi_ssid.clone(),
                password: request.wifi_password.clone(),
            },
            render_mode: RenderMode::Local,
        },
        api_key: request.api_key.clone(),
        cert_pem: cert_pem.clone(),
        cert_fingerprint: cert_fingerprint.clone(),
    };
    save_persisted_state(&runtime.storage_dir, &persisted).map_err(|err| {
        ResponseError::new(StatusCode::INTERNAL_SERVER_ERROR, "persist_failed", err.to_string())
    })?;

    runtime.persisted = Some(persisted.clone());
    runtime.device = DeviceState::from_persisted(StartupMode::Bootstrap, Some(&persisted));
    runtime.device.network_ready = false;

    Ok(Json(InitResponse {
        nickname: persisted.config.nickname.clone(),
        base_url: runtime.base_url.clone(),
        api_key: persisted.api_key,
        cert_pem,
        cert_fingerprint,
    }))
}

async fn get_state(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeviceState>, ResponseError> {
    let runtime = state.runtime.lock().expect("state lock poisoned");
    authorize(&runtime, &headers)?;
    runtime.require_wifi_ready()?;
    Ok(Json(runtime.device.clone()))
}

async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<lilbug_core::DeviceConfig>, ResponseError> {
    let runtime = state.runtime.lock().expect("state lock poisoned");
    authorize(&runtime, &headers)?;
    runtime.require_wifi_ready()?;
    Ok(Json(runtime.device.config.clone()))
}

async fn update_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(patch): Json<ConfigPatchRequest>,
) -> Result<Json<lilbug_core::DeviceConfig>, ResponseError> {
    let mut runtime = state.runtime.lock().expect("state lock poisoned");
    authorize(&runtime, &headers)?;
    runtime.require_wifi_ready()?;

    runtime.device.apply_config_patch(&patch);
    let updated_config = runtime.device.config.clone();
    let storage_dir = runtime.storage_dir.clone();
    let persisted = runtime.persisted.as_mut().ok_or_else(|| {
        ResponseError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "missing_state",
            "persisted state is unavailable",
        )
    })?;
    persisted.config = updated_config.clone();
    save_persisted_state(&storage_dir, persisted).map_err(|err| {
        ResponseError::new(StatusCode::INTERNAL_SERVER_ERROR, "persist_failed", err.to_string())
    })?;

    Ok(Json(updated_config))
}

async fn run_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(command): Json<CommandRequest>,
) -> Result<Json<DeviceState>, ResponseError> {
    let mut runtime = state.runtime.lock().expect("state lock poisoned");
    authorize(&runtime, &headers)?;
    runtime.require_wifi_ready()?;

    runtime.device.apply_command(command).map_err(|message| {
        ResponseError::new(StatusCode::BAD_REQUEST, "invalid_command", message)
    })?;
    Ok(Json(runtime.device.clone()))
}

async fn get_frame(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ResponseError> {
    let runtime = state.runtime.lock().expect("state lock poisoned");
    authorize(&runtime, &headers)?;
    runtime.require_wifi_ready()?;

    let png = render_frame_png(&runtime.device).map_err(|err| {
        ResponseError::new(StatusCode::INTERNAL_SERVER_ERROR, "frame_encode_failed", err)
    })?;

    Ok(([("content-type", "image/png")], png))
}

fn authorize(runtime: &RuntimeState, headers: &HeaderMap) -> Result<(), ResponseError> {
    let expected = runtime
        .persisted
        .as_ref()
        .ok_or_else(|| ResponseError::new(StatusCode::UNAUTHORIZED, "missing_token", "device is not provisioned"))?
        .api_key
        .clone();
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ResponseError::new(
                StatusCode::UNAUTHORIZED,
                "missing_token",
                "missing Authorization: Bearer <api_key> header",
            )
        })?;

    match header.strip_prefix("Bearer ") {
        Some(token) if token == expected => Ok(()),
        _ => Err(ResponseError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "invalid bearer token",
        )),
    }
}

fn load_persisted_state(storage_dir: &Path) -> Result<Option<PersistedDeviceState>> {
    let path = persisted_path(storage_dir);
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let persisted = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(persisted))
}

fn save_persisted_state(storage_dir: &Path, persisted: &PersistedDeviceState) -> Result<()> {
    fs::create_dir_all(storage_dir)
        .with_context(|| format!("failed to create {}", storage_dir.display()))?;
    let path = persisted_path(storage_dir);
    let body = serde_json::to_string_pretty(persisted).context("failed to encode persisted state")?;
    fs::write(&path, format!("{body}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

struct GeneratedCerts {
    cert_pem: String,
    key_pem: String,
}

fn load_or_create_certs(storage_dir: &Path) -> Result<GeneratedCerts> {
    fs::create_dir_all(storage_dir)
        .with_context(|| format!("failed to create {}", storage_dir.display()))?;

    let cert_path = cert_path(storage_dir);
    let key_path = key_path(storage_dir);
    if cert_path.exists() && key_path.exists() {
        return Ok(GeneratedCerts {
            cert_pem: fs::read_to_string(&cert_path)
                .with_context(|| format!("failed to read {}", cert_path.display()))?,
            key_pem: fs::read_to_string(&key_path)
                .with_context(|| format!("failed to read {}", key_path.display()))?,
        });
    }

    let cert = generate_simple_self_signed(vec!["localhost".to_string()])
        .context("failed to generate self-signed certificate")?;
    let cert_pem = cert.serialize_pem().context("failed to serialize certificate pem")?;
    let key_pem = cert.serialize_private_key_pem();
    fs::write(&cert_path, &cert_pem)
        .with_context(|| format!("failed to write {}", cert_path.display()))?;
    fs::write(&key_path, &key_pem)
        .with_context(|| format!("failed to write {}", key_path.display()))?;

    Ok(GeneratedCerts { cert_pem, key_pem })
}

fn persisted_path(storage_dir: &Path) -> PathBuf {
    storage_dir.join("device-state.json")
}

fn cert_path(storage_dir: &Path) -> PathBuf {
    storage_dir.join("cert.pem")
}

fn key_path(storage_dir: &Path) -> PathBuf {
    storage_dir.join("key.pem")
}

fn default_base_url(mode: StartupMode) -> String {
    match mode {
        StartupMode::Bootstrap => DEFAULT_WIFI_URL.to_string(),
        StartupMode::Wifi => DEFAULT_WIFI_URL.to_string(),
    }
}

fn run_headless(run_for_ms: Option<u64>) -> Result<()> {
    let started = Instant::now();
    loop {
        if let Some(limit_ms) = run_for_ms {
            if started.elapsed() >= Duration::from_millis(limit_ms) {
                break;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn run_window(state: AppState, run_for_ms: Option<u64>) -> Result<()> {
    let mut window = Window::new(
        "lilbug emulator",
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        WindowOptions::default(),
    )
    .context("failed to create emulator window")?;
    window.set_target_fps(60);

    let mut buffer = vec![0_u32; WINDOW_WIDTH * WINDOW_HEIGHT];
    let started = Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if let Some(limit_ms) = run_for_ms {
            if started.elapsed() >= Duration::from_millis(limit_ms) {
                break;
            }
        }

        let snapshot = state.runtime.lock().expect("state lock poisoned").device.clone();
        draw_scene(&mut buffer, &snapshot);
        window
            .update_with_buffer(&buffer, WINDOW_WIDTH, WINDOW_HEIGHT)
            .context("failed to present emulator frame")?;
    }

    Ok(())
}

fn render_frame_png(state: &DeviceState) -> Result<Vec<u8>, String> {
    let mut buffer = vec![0_u32; WINDOW_WIDTH * WINDOW_HEIGHT];
    draw_scene(&mut buffer, state);

    let mut rgba = Vec::with_capacity(WINDOW_WIDTH * WINDOW_HEIGHT * 4);
    for pixel in buffer {
        rgba.push(((pixel >> 16) & 0xFF) as u8);
        rgba.push(((pixel >> 8) & 0xFF) as u8);
        rgba.push((pixel & 0xFF) as u8);
        rgba.push(0xFF);
    }

    let mut png = Vec::new();
    let mut encoder = Encoder::new(&mut png, WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|err| format!("failed to start png encoding: {err}"))?;
    writer
        .write_image_data(&rgba)
        .map_err(|err| format!("failed to write png data: {err}"))?;
    drop(writer);
    Ok(png)
}

fn draw_scene(buffer: &mut [u32], state: &DeviceState) {
    buffer.fill(COLOR_BG);

    let center_x = (WINDOW_WIDTH / 2) as i32;
    let center_y = (DISPLAY_SIZE / 2) as i32;
    let radius = (DISPLAY_SIZE as i32 / 2) - 10;
    let face_color = match state.face {
        FaceExpression::Happy => COLOR_FACE_HAPPY,
        FaceExpression::Blink => COLOR_FACE_BLINK,
        FaceExpression::Surprised => COLOR_FACE_SURPRISED,
        FaceExpression::Neutral => COLOR_FACE_NEUTRAL,
    };

    for y in 0..DISPLAY_SIZE as i32 {
        for x in 0..WINDOW_WIDTH as i32 {
            let dx = x - center_x;
            let dy = y - center_y;
            let distance_sq = dx * dx + dy * dy;
            let outer = radius * radius;
            let inner = (radius - 4) * (radius - 4);
            let color = if distance_sq <= inner {
                face_color
            } else if distance_sq <= outer {
                COLOR_RING
            } else {
                COLOR_BG
            };
            put_pixel(buffer, x, y, color);
        }
    }

    draw_face_features(buffer, state.face);

    let forward_color = if state.motor == MotorDirection::Forward {
        COLOR_TEXT_ACTIVE
    } else {
        COLOR_TEXT_DIM
    };
    let backward_color = if state.motor == MotorDirection::Backward {
        COLOR_TEXT_ACTIVE
    } else {
        COLOR_TEXT_DIM
    };

    draw_text(buffer, 12, 442, "[FORWARD]", forward_color, 2);
    draw_text(buffer, 278, 442, "[BACKWARD]", backward_color, 2);
    draw_text(buffer, 118, 425, &format!("face:{}", state.face.as_str()), COLOR_WHITE, 1);
}

fn draw_face_features(buffer: &mut [u32], face: FaceExpression) {
    match face {
        FaceExpression::Blink => {
            draw_rect(buffer, 130, 150, 52, 6, COLOR_WHITE);
            draw_rect(buffer, 230, 150, 52, 6, COLOR_WHITE);
            draw_rect(buffer, 164, 272, 84, 6, COLOR_WHITE);
        }
        FaceExpression::Surprised => {
            draw_rect(buffer, 145, 135, 22, 34, COLOR_WHITE);
            draw_rect(buffer, 245, 135, 22, 34, COLOR_WHITE);
            draw_ring(buffer, 206, 266, 22, 5, COLOR_WHITE);
        }
        FaceExpression::Happy => {
            draw_rect(buffer, 145, 135, 22, 34, COLOR_WHITE);
            draw_rect(buffer, 245, 135, 22, 34, COLOR_WHITE);
            draw_rect(buffer, 156, 272, 102, 6, COLOR_WHITE);
            draw_rect(buffer, 170, 278, 74, 6, COLOR_WHITE);
        }
        FaceExpression::Neutral => {
            draw_rect(buffer, 145, 135, 22, 34, COLOR_WHITE);
            draw_rect(buffer, 245, 135, 22, 34, COLOR_WHITE);
            draw_rect(buffer, 160, 272, 92, 6, COLOR_WHITE);
        }
    }
}

fn draw_ring(buffer: &mut [u32], cx: i32, cy: i32, radius: i32, thickness: i32, color: u32) {
    let outer = radius * radius;
    let inner = (radius - thickness) * (radius - thickness);
    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            let dx = x - cx;
            let dy = y - cy;
            let dist = dx * dx + dy * dy;
            if dist <= outer && dist >= inner {
                put_pixel(buffer, x, y, color);
            }
        }
    }
}

fn draw_rect(buffer: &mut [u32], x: i32, y: i32, width: i32, height: i32, color: u32) {
    for yy in y..(y + height) {
        for xx in x..(x + width) {
            put_pixel(buffer, xx, yy, color);
        }
    }
}

fn draw_text(buffer: &mut [u32], x: i32, y: i32, text: &str, color: u32, scale: i32) {
    let mut cursor_x = x;
    for ch in text.chars() {
        if let Some(glyph) = BASIC_FONTS.get(ch) {
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if bits & (1 << col) != 0 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                put_pixel(
                                    buffer,
                                    cursor_x + (col * scale) + sx,
                                    y + (row as i32 * scale) + sy,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
        }
        cursor_x += 8 * scale;
    }
}

fn put_pixel(buffer: &mut [u32], x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 || x >= WINDOW_WIDTH as i32 || y >= WINDOW_HEIGHT as i32 {
        return;
    }
    let index = y as usize * WINDOW_WIDTH + x as usize;
    buffer[index] = color;
}

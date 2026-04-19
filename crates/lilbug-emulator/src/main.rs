use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use font8x8::{BASIC_FONTS, UnicodeFonts};
use lilbug_core::{
    ApiError, CommandRequest, ConfigPatchRequest, DISPLAY_SIZE, DeviceState, FaceExpression,
    InitRequest, InitResponse, MotorDirection, PersistedDeviceState, RenderMode, StartupMode,
    WINDOW_HEIGHT, sha256_fingerprint,
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
    motion_deadline: Option<Instant>,
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

    fn tick(&mut self) {
        let now = Instant::now();
        if let Some(deadline) = self.motion_deadline {
            if now >= deadline {
                self.motion_deadline = None;
                self.device.motion.direction = MotorDirection::Stop;
                self.device.active_motion_deadline_ms = None;
            } else {
                let remaining = deadline.saturating_duration_since(now).as_millis();
                self.device.active_motion_deadline_ms =
                    Some(remaining.min(u128::from(u64::MAX)) as u64);
            }
        } else {
            self.device.active_motion_deadline_ms = None;
        }
    }

    fn apply_command(&mut self, command: CommandRequest) -> Result<(), String> {
        self.device.apply_command(command.clone())?;
        self.motion_deadline = match command.command.as_str() {
            "forward" | "backward" => command
                .duration_ms
                .map(|duration_ms| Instant::now() + Duration::from_millis(duration_ms)),
            "stop" | "brake" => None,
            "face" => self.motion_deadline,
            _ => None,
        };
        self.tick();
        Ok(())
    }

    fn snapshot(&mut self) -> DeviceState {
        self.tick();
        self.device.clone()
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

fn main() -> Result<()> {
    let args = Args::parse();
    let certs = load_or_create_certs(&args.storage_dir, &collect_certificate_subject_names(&args))?;
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
        .unwrap_or_else(|| default_base_url(&args.https_addr));
    let runtime = Arc::new(Mutex::new(RuntimeState {
        device: DeviceState::from_persisted(args.mode, persisted.as_ref()),
        persisted,
        storage_dir: args.storage_dir.clone(),
        base_url,
        motion_deadline: None,
    }));
    let app_state = AppState { runtime };

    let addr: SocketAddr = args
        .https_addr
        .parse()
        .with_context(|| format!("invalid --https-addr {}", args.https_addr))?;
    ensure_addr_available(addr)?;
    let handle = Handle::new();
    let (startup_tx, startup_rx) = mpsc::sync_channel::<Result<()>>(1);
    let server_state = app_state.clone();
    let server_handle = handle.clone();
    let cert_pem = certs.cert_pem.into_bytes();
    let key_pem = certs.key_pem.into_bytes();

    let server_thread = thread::spawn(move || -> Result<()> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to build tokio runtime")?;

        runtime.block_on(async move {
            let tls = RustlsConfig::from_pem(cert_pem, key_pem)
                .await
                .context("failed to build rustls config")?;
            let _ = startup_tx.send(Ok(()));

            let ticker = tokio::spawn(run_runtime_tick(server_state.clone()));
            let server_result = run_server(addr, tls, server_handle, server_state).await;
            ticker.abort();
            let _ = ticker.await;
            server_result
        })
    });

    match startup_rx.recv() {
        Ok(result) => result?,
        Err(err) => {
            return Err(anyhow!("failed to receive emulator server startup status: {err}"));
        }
    }

    let ui_result = if args.headless {
        run_headless(args.run_for_ms)
    } else {
        run_window(app_state.clone(), args.run_for_ms)
    };

    handle.graceful_shutdown(Some(Duration::from_secs(1)));
    match server_thread.join() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(err.context("emulator server thread failed")),
        Err(_) => return Err(anyhow!("emulator server thread panicked")),
    }

    ui_result
}

async fn run_runtime_tick(state: AppState) -> Result<()> {
    loop {
        {
            let mut runtime = state.runtime.lock().expect("state lock poisoned");
            runtime.tick();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn run_server(addr: SocketAddr, tls: RustlsConfig, handle: Handle, state: AppState) -> Result<()> {
    let app = Router::new()
        .route("/v1/init", post(init_device))
        .route("/v1/state", get(get_state))
        .route("/v1/config", get(get_config).post(update_config))
        .route("/v1/cmd", post(run_command))
        .route("/v1/frame.png", get(get_frame))
        .with_state(state);

    axum_server::bind_rustls(addr, tls)
        .handle(handle)
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

    let cert_pem = fs::read_to_string(cert_path(&runtime.storage_dir)).map_err(|err| {
        ResponseError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io_error",
            err.to_string(),
        )
    })?;
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
        ResponseError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persist_failed",
            err.to_string(),
        )
    })?;

    runtime.persisted = Some(persisted.clone());
    runtime.device = DeviceState::from_persisted(StartupMode::Bootstrap, Some(&persisted));
    runtime.motion_deadline = None;
    runtime.device.network_ready = false;
    runtime.tick();

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
    let mut runtime = state.runtime.lock().expect("state lock poisoned");
    authorize(&runtime, &headers)?;
    runtime.require_wifi_ready()?;
    Ok(Json(runtime.snapshot()))
}

async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<lilbug_core::DeviceConfig>, ResponseError> {
    let mut runtime = state.runtime.lock().expect("state lock poisoned");
    authorize(&runtime, &headers)?;
    runtime.require_wifi_ready()?;
    runtime.tick();
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
    runtime.tick();
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
        ResponseError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persist_failed",
            err.to_string(),
        )
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

    runtime.apply_command(command).map_err(|message| {
        ResponseError::new(StatusCode::BAD_REQUEST, "invalid_command", message)
    })?;
    Ok(Json(runtime.snapshot()))
}

async fn get_frame(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ResponseError> {
    let mut runtime = state.runtime.lock().expect("state lock poisoned");
    authorize(&runtime, &headers)?;
    runtime.require_wifi_ready()?;

    let snapshot = runtime.snapshot();
    let png = render_frame_png(&snapshot).map_err(|err| {
        ResponseError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "frame_encode_failed",
            err,
        )
    })?;

    Ok(([("content-type", "image/png")], png))
}

fn authorize(runtime: &RuntimeState, headers: &HeaderMap) -> Result<(), ResponseError> {
    let expected = runtime
        .persisted
        .as_ref()
        .ok_or_else(|| {
            ResponseError::new(
                StatusCode::UNAUTHORIZED,
                "missing_token",
                "device is not provisioned",
            )
        })?
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

    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let persisted = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(persisted))
}

fn save_persisted_state(storage_dir: &Path, persisted: &PersistedDeviceState) -> Result<()> {
    fs::create_dir_all(storage_dir)
        .with_context(|| format!("failed to create {}", storage_dir.display()))?;
    let path = persisted_path(storage_dir);
    let body =
        serde_json::to_string_pretty(persisted).context("failed to encode persisted state")?;
    fs::write(&path, format!("{body}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

struct GeneratedCerts {
    cert_pem: String,
    key_pem: String,
}

fn load_or_create_certs(storage_dir: &Path, subject_names: &[String]) -> Result<GeneratedCerts> {
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

    let cert = generate_simple_self_signed(subject_names.to_vec())
        .context("failed to generate self-signed certificate")?;
    let cert_pem = cert
        .serialize_pem()
        .context("failed to serialize certificate pem")?;
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

fn ensure_addr_available(addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(addr)
        .with_context(|| format!("failed to bind emulator HTTPS address {addr}"))?;
    drop(listener);
    Ok(())
}

fn default_base_url(https_addr: &str) -> String {
    let host = parse_host_from_socket_addr(https_addr)
        .filter(|host| *host != "0.0.0.0" && *host != "::")
        .unwrap_or("localhost");
    format!("https://{host}:8443")
}

fn collect_certificate_subject_names(args: &Args) -> Vec<String> {
    let mut names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];

    if let Some(host) = parse_host_from_socket_addr(&args.https_addr) {
        push_unique(&mut names, host.to_string());
    }
    if let Some(base_url) = &args.wifi_base_url {
        if let Some(host) = parse_host_from_base_url(base_url) {
            push_unique(&mut names, host.to_string());
        }
    }

    names
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn parse_host_from_base_url(base_url: &str) -> Option<&str> {
    let after_scheme = base_url.split_once("://")?.1;
    let authority = after_scheme.split('/').next()?;
    if authority.starts_with('[') {
        authority
            .split(']')
            .next()
            .map(|value| value.trim_start_matches('['))
    } else {
        authority.split(':').next()
    }
}

fn parse_host_from_socket_addr(value: &str) -> Option<&str> {
    if value.starts_with('[') {
        value
            .split(']')
            .next()
            .map(|host| host.trim_start_matches('['))
    } else {
        value.rsplit_once(':').map(|(host, _)| host)
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

        let snapshot = state
            .runtime
            .lock()
            .expect("state lock poisoned")
            .snapshot();
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
    let face_color = match state.face.expression {
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

    draw_face_features(buffer, state.face.expression);

    let forward_color = if state.motion.direction == MotorDirection::Forward {
        COLOR_TEXT_ACTIVE
    } else {
        COLOR_TEXT_DIM
    };
    let backward_color = if state.motion.direction == MotorDirection::Backward {
        COLOR_TEXT_ACTIVE
    } else {
        COLOR_TEXT_DIM
    };

    draw_text(buffer, 12, 442, "[FORWARD]", forward_color, 2);
    draw_text(buffer, 236, 442, "[BACKWARD]", backward_color, 2);
    draw_text(
        buffer,
        88,
        425,
        &format!(
            "face:{} motion:{:?}",
            state.face.expression.as_str(),
            state.motion.direction
        ),
        COLOR_WHITE,
        1,
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_state() -> RuntimeState {
        let persisted = PersistedDeviceState {
            config: lilbug_core::DeviceConfig {
                nickname: "bug-02".to_string(),
                wifi: lilbug_core::WifiConfig {
                    ssid: "lab-net".to_string(),
                    password: "secret".to_string(),
                },
                render_mode: RenderMode::Local,
            },
            api_key: "lb_test".to_string(),
            cert_pem: "pem".to_string(),
            cert_fingerprint: "SHA256:TEST".to_string(),
        };

        RuntimeState {
            device: DeviceState::from_persisted(StartupMode::Wifi, Some(&persisted)),
            persisted: Some(persisted),
            storage_dir: PathBuf::from("/tmp/lilbug-test"),
            base_url: "https://127.0.0.1:8443".to_string(),
            motion_deadline: None,
        }
    }

    #[test]
    fn timed_motion_expires_to_stop() {
        let mut runtime = runtime_state();

        runtime
            .apply_command(CommandRequest {
                command: "forward".to_string(),
                duration_ms: Some(5),
                value: None,
            })
            .unwrap();
        thread::sleep(Duration::from_millis(10));
        runtime.tick();

        assert_eq!(runtime.device.motion.direction, MotorDirection::Stop);
        assert_eq!(runtime.device.active_motion_deadline_ms, None);
    }

    #[test]
    fn face_command_preserves_existing_motion_deadline() {
        let mut runtime = runtime_state();

        runtime
            .apply_command(CommandRequest {
                command: "forward".to_string(),
                duration_ms: Some(100),
                value: None,
            })
            .unwrap();
        let first_deadline = runtime.motion_deadline;

        runtime
            .apply_command(CommandRequest {
                command: "face".to_string(),
                duration_ms: None,
                value: Some("happy".to_string()),
            })
            .unwrap();

        assert_eq!(runtime.device.motion.direction, MotorDirection::Forward);
        assert_eq!(runtime.device.face.expression, FaceExpression::Happy);
        assert_eq!(runtime.motion_deadline, first_deadline);
    }

    #[test]
    fn authorize_accepts_matching_bearer_token() {
        let runtime = runtime_state();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer lb_test".parse().unwrap(),
        );

        assert!(authorize(&runtime, &headers).is_ok());
    }

    #[test]
    fn authorize_rejects_invalid_bearer_token() {
        let runtime = runtime_state();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer wrong".parse().unwrap(),
        );

        let error = authorize(&runtime, &headers).unwrap_err();
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        assert_eq!(error.body.code, "invalid_token");
    }
}

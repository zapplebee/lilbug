use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use lilbug_core::{
    ApiError, CliConfig, CommandRequest, ConfigPatchRequest, DEFAULT_BOOTSTRAP_URL, DeviceConfig,
    InitRequest, KnownDevice, parse_command_token, sha256_fingerprint,
};
use reqwest::{Certificate, Client, Method};

#[derive(Parser, Debug)]
#[command(
    name = "lilbug",
    about = "CLI for the lilbug rev1 emulator and future device"
)]
struct Cli {
    #[arg(long)]
    config_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init {
        #[arg(long)]
        nickname: String,
        #[arg(long, default_value = DEFAULT_BOOTSTRAP_URL)]
        bootstrap_url: String,
        #[arg(long)]
        wifi_ssid: String,
        #[arg(long)]
        wifi_password: String,
        #[arg(long)]
        api_key: Option<String>,
    },
    State {
        #[arg(long)]
        nickname: String,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Cmd {
        #[arg(long)]
        nickname: String,
        token: String,
    },
    Frame {
        #[arg(long)]
        nickname: String,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    Get {
        #[arg(long)]
        nickname: String,
    },
    Set {
        #[arg(long)]
        nickname: String,
        field: String,
        value: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = match cli.config_path {
        Some(path) => path,
        None => CliConfig::path().map_err(anyhow::Error::msg)?,
    };

    match cli.command {
        Commands::Init {
            nickname,
            bootstrap_url,
            wifi_ssid,
            wifi_password,
            api_key,
        } => {
            let api_key = api_key.unwrap_or_else(generate_api_key);
            let mut config = CliConfig::load(&config_path).map_err(anyhow::Error::msg)?;
            let response = bootstrap_init(
                &bootstrap_url,
                InitRequest {
                    nickname: nickname.clone(),
                    wifi_ssid,
                    wifi_password,
                    api_key,
                },
            )
            .await?;

            verify_pem_fingerprint(&response.cert_pem, &response.cert_fingerprint)?;
            config.upsert_device_for_target(
                nickname,
                KnownDevice {
                    base_url: response.base_url.clone(),
                    api_key: response.api_key.clone(),
                    cert_fingerprint: response.cert_fingerprint.clone(),
                    cert_pem: Some(response.cert_pem.clone()),
                },
            );
            config.save(&config_path).map_err(anyhow::Error::msg)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response)
                    .context("failed to render init response")?
            );
        }
        Commands::State { nickname } => {
            let device = load_known_device(&config_path, &nickname)?;
            let state: serde_json::Value =
                send_json_no_body(Method::GET, &device, "/v1/state").await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&state).context("failed to render state")?
            );
        }
        Commands::Config { command } => match command {
            ConfigCommand::Get { nickname } => {
                let device = load_known_device(&config_path, &nickname)?;
                let config: serde_json::Value =
                    send_json_no_body(Method::GET, &device, "/v1/config").await?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&config)
                        .context("failed to render config response")?
                );
            }
            ConfigCommand::Set {
                nickname,
                field,
                value,
            } => {
                let mut cli_config = CliConfig::load(&config_path).map_err(anyhow::Error::msg)?;
                let device = cli_config
                    .get_device(&nickname)
                    .map_err(anyhow::Error::msg)?
                    .clone();
                let patch = patch_from_cli(&field, value)?;
                let config: DeviceConfig =
                    send_json(Method::POST, &device, "/v1/config", Some(&patch)).await?;

                if field == "nickname" {
                    // The local config file is keyed by nickname, so a successful remote rename
                    // must move the stored trust record to the new nickname as well.
                    cli_config
                        .rename_device(&nickname, config.nickname.clone())
                        .map_err(anyhow::Error::msg)?;
                    cli_config.save(&config_path).map_err(anyhow::Error::msg)?;
                }

                println!(
                    "{}",
                    serde_json::to_string_pretty(&config)
                        .context("failed to render config response")?
                );
            }
        },
        Commands::Cmd { nickname, token } => {
            let device = load_known_device(&config_path, &nickname)?;
            let command: CommandRequest =
                parse_command_token(&token).map_err(anyhow::Error::msg)?;
            let state: serde_json::Value =
                send_json(Method::POST, &device, "/v1/cmd", Some(&command)).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&state).context("failed to render state response")?
            );
        }
        Commands::Frame { nickname, out } => {
            let device = load_known_device(&config_path, &nickname)?;
            let bytes = send_bytes(&device, "/v1/frame.png").await?;
            fs::write(&out, &bytes)
                .with_context(|| format!("failed to write {}", out.display()))?;
            println!("wrote {} ({} bytes)", out.display(), bytes.len());
        }
    }

    Ok(())
}

async fn bootstrap_init(url: &str, request: InitRequest) -> Result<lilbug_core::InitResponse> {
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .context("failed to build bootstrap HTTPS client")?;

    let response = client
        .post(format!("{url}/v1/init"))
        .json(&request)
        .send()
        .await
        .with_context(|| format!("failed to call {url}/v1/init"))?;

    if !response.status().is_success() {
        let status = response.status();
        let api_error = response.json::<ApiError>().await.ok();
        let message = api_error
            .map(|err| format!("{}: {}", err.code, err.message))
            .unwrap_or_else(|| format!("bootstrap request failed with {status}"));
        bail!(message);
    }

    response
        .json()
        .await
        .context("failed to decode init response")
}

fn load_known_device(config_path: &PathBuf, nickname: &str) -> Result<KnownDevice> {
    let config = CliConfig::load(config_path).map_err(anyhow::Error::msg)?;
    let device = config
        .get_device(nickname)
        .map_err(anyhow::Error::msg)?
        .clone();
    if let Some(cert_pem) = &device.cert_pem {
        verify_pem_fingerprint(cert_pem, &device.cert_fingerprint)?;
    }
    Ok(device)
}

fn patch_from_cli(field: &str, value: String) -> Result<ConfigPatchRequest> {
    match field {
        "nickname" => Ok(ConfigPatchRequest {
            nickname: Some(value),
            ..ConfigPatchRequest::default()
        }),
        "wifi.ssid" => Ok(ConfigPatchRequest {
            wifi_ssid: Some(value),
            ..ConfigPatchRequest::default()
        }),
        "wifi.password" => Ok(ConfigPatchRequest {
            wifi_password: Some(value),
            ..ConfigPatchRequest::default()
        }),
        "render_mode" => Ok(ConfigPatchRequest {
            render_mode: Some(match value.as_str() {
                "local" => lilbug_core::RenderMode::Local,
                "streamed_override" => lilbug_core::RenderMode::StreamedOverride,
                other => {
                    bail!("unsupported render_mode '{other}'; expected local or streamed_override")
                }
            }),
            ..ConfigPatchRequest::default()
        }),
        other => bail!(
            "unsupported config field '{other}'; expected nickname, wifi.ssid, wifi.password, or render_mode"
        ),
    }
}

async fn send_json<T, B>(
    method: Method,
    device: &KnownDevice,
    route: &str,
    body: Option<&B>,
) -> Result<T>
where
    T: serde::de::DeserializeOwned,
    B: serde::Serialize + ?Sized,
{
    let client = client_for_device(device)?;
    let request = client
        .request(method, format!("{}{}", device.base_url, route))
        .bearer_auth(&device.api_key);
    let request = if let Some(body) = body {
        request.json(body)
    } else {
        request
    };
    let response = request
        .send()
        .await
        .with_context(|| format!("failed to call {}{}", device.base_url, route))?;

    decode_json_response(response).await
}

async fn send_json_no_body<T>(method: Method, device: &KnownDevice, route: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    send_json::<T, ()>(method, device, route, None).await
}

async fn send_bytes(device: &KnownDevice, route: &str) -> Result<Vec<u8>> {
    let client = client_for_device(device)?;
    let response = client
        .get(format!("{}{}", device.base_url, route))
        .bearer_auth(&device.api_key)
        .send()
        .await
        .with_context(|| format!("failed to call {}{}", device.base_url, route))?;

    if !response.status().is_success() {
        let status = response.status();
        let api_error = response.json::<ApiError>().await.ok();
        let message = api_error
            .map(|err| format!("{}: {}", err.code, err.message))
            .unwrap_or_else(|| format!("request failed with {status}"));
        bail!(message);
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .context("failed to read binary response")
}

async fn decode_json_response<T>(response: reqwest::Response) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    if !response.status().is_success() {
        let status = response.status();
        let api_error = response.json::<ApiError>().await.ok();
        let message = api_error
            .map(|err| format!("{}: {}", err.code, err.message))
            .unwrap_or_else(|| format!("request failed with {status}"));
        bail!(message);
    }

    response
        .json()
        .await
        .context("failed to decode JSON response")
}

fn client_for_device(device: &KnownDevice) -> Result<Client> {
    let cert_pem = device
        .cert_pem
        .as_deref()
        .ok_or_else(|| anyhow!("device record is missing cert_pem trust material"))?;
    let certificate = Certificate::from_pem(cert_pem.as_bytes())
        .context("failed to parse stored device certificate")?;
    Client::builder()
        .add_root_certificate(certificate)
        .build()
        .context("failed to build HTTPS client")
}

fn verify_pem_fingerprint(cert_pem: &str, expected: &str) -> Result<()> {
    let actual = sha256_fingerprint(cert_pem.as_bytes());
    if actual != expected {
        bail!(
            "certificate fingerprint mismatch: expected {}, got {}",
            expected,
            actual
        );
    }
    Ok(())
}

fn generate_api_key() -> String {
    format!("lb_{:016x}", rand::random::<u64>())
}

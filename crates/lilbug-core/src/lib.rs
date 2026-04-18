use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DISPLAY_SIZE: usize = 412;
pub const WINDOW_HEIGHT: usize = 480;
pub const DEFAULT_BOOTSTRAP_URL: &str = "https://localhost:7443";
pub const DEFAULT_WIFI_URL: &str = "https://localhost:8443";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StartupMode {
    #[default]
    Bootstrap,
    Wifi,
}

impl FromStr for StartupMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "bootstrap" => Ok(Self::Bootstrap),
            "wifi" => Ok(Self::Wifi),
            other => Err(format!("unsupported startup mode '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RenderMode {
    #[default]
    Local,
    StreamedOverride,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MotorDirection {
    #[default]
    Stop,
    Forward,
    Backward,
    Brake,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FaceExpression {
    #[default]
    Neutral,
    Happy,
    Blink,
    Surprised,
}

impl FaceExpression {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Happy => "happy",
            Self::Blink => "blink",
            Self::Surprised => "surprised",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "neutral" => Ok(Self::Neutral),
            "happy" => Ok(Self::Happy),
            "blink" => Ok(Self::Blink),
            "surprised" => Ok(Self::Surprised),
            other => Err(format!(
                "unsupported face expression '{other}'; expected neutral, happy, blink, or surprised"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WifiConfig {
    pub ssid: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceConfig {
    pub nickname: String,
    pub wifi: WifiConfig,
    pub render_mode: RenderMode,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            nickname: "lilbug-emulator".to_string(),
            wifi: WifiConfig::default(),
            render_mode: RenderMode::Local,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedDeviceState {
    pub config: DeviceConfig,
    pub api_key: String,
    pub cert_pem: String,
    pub cert_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceState {
    pub mode: StartupMode,
    pub provisioned: bool,
    pub config: DeviceConfig,
    pub face: FaceExpression,
    pub motor: MotorDirection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_command: Option<CommandRequest>,
    pub network_ready: bool,
}

impl DeviceState {
    pub fn from_persisted(mode: StartupMode, persisted: Option<&PersistedDeviceState>) -> Self {
        match persisted {
            Some(persisted) => Self {
                mode,
                provisioned: true,
                config: persisted.config.clone(),
                face: FaceExpression::Neutral,
                motor: MotorDirection::Stop,
                last_command: None,
                network_ready: mode == StartupMode::Wifi,
            },
            None => Self {
                mode,
                provisioned: false,
                config: DeviceConfig::default(),
                face: FaceExpression::Neutral,
                motor: MotorDirection::Stop,
                last_command: None,
                network_ready: false,
            },
        }
    }

    pub fn apply_config_patch(&mut self, patch: &ConfigPatchRequest) {
        if let Some(nickname) = &patch.nickname {
            self.config.nickname = nickname.clone();
        }
        if let Some(ssid) = &patch.wifi_ssid {
            self.config.wifi.ssid = ssid.clone();
        }
        if let Some(password) = &patch.wifi_password {
            self.config.wifi.password = password.clone();
        }
        if let Some(render_mode) = patch.render_mode {
            self.config.render_mode = render_mode;
        }
    }

    pub fn apply_command(&mut self, command: CommandRequest) -> Result<(), String> {
        command.validate()?;
        match command.command.as_str() {
            "forward" => self.motor = MotorDirection::Forward,
            "backward" => self.motor = MotorDirection::Backward,
            "stop" => self.motor = MotorDirection::Stop,
            "brake" => self.motor = MotorDirection::Brake,
            "face" => {
                let expression = FaceExpression::parse(
                    command.value.as_deref().ok_or("face command requires a value")?,
                )?;
                self.face = expression;
            }
            _ => return Err(format!("unsupported command '{}'", command.command)),
        }
        self.last_command = Some(command);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitRequest {
    pub nickname: String,
    pub wifi_ssid: String,
    pub wifi_password: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitResponse {
    pub nickname: String,
    pub base_url: String,
    pub api_key: String,
    pub cert_pem: String,
    pub cert_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConfigPatchRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wifi_ssid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wifi_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_mode: Option<RenderMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandRequest {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl CommandRequest {
    pub fn validate(&self) -> Result<(), String> {
        match self.command.as_str() {
            "forward" | "backward" => {
                if self.duration_ms.is_none() {
                    return Err(format!("{} command requires duration_ms", self.command));
                }
                if self.value.is_some() {
                    return Err(format!("{} command does not accept value", self.command));
                }
                Ok(())
            }
            "stop" | "brake" => {
                if self.duration_ms.is_some() || self.value.is_some() {
                    return Err(format!("{} command does not take arguments", self.command));
                }
                Ok(())
            }
            "face" => {
                let value = self
                    .value
                    .as_deref()
                    .ok_or("face command requires value")?;
                if self.duration_ms.is_some() {
                    return Err("face command does not accept duration_ms".to_string());
                }
                FaceExpression::parse(value).map(|_| ())
            }
            other => Err(format!(
                "unsupported command '{other}'; expected forward, backward, stop, brake, or face"
            )),
        }
    }
}

pub fn parse_command_token(token: &str) -> Result<CommandRequest, String> {
    let mut parts = token.splitn(2, ':');
    let command = parts.next().unwrap_or_default();
    let value = parts.next();

    let parsed = match command {
        "fwd" => CommandRequest {
            command: "forward".to_string(),
            duration_ms: Some(parse_duration(value, token)?),
            value: None,
        },
        "back" => CommandRequest {
            command: "backward".to_string(),
            duration_ms: Some(parse_duration(value, token)?),
            value: None,
        },
        "stop" => require_no_value(token, value).map(|_| CommandRequest {
            command: "stop".to_string(),
            duration_ms: None,
            value: None,
        })?,
        "brake" => require_no_value(token, value).map(|_| CommandRequest {
            command: "brake".to_string(),
            duration_ms: None,
            value: None,
        })?,
        "face" => CommandRequest {
            command: "face".to_string(),
            duration_ms: None,
            value: Some(
                value
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| format!("invalid command token '{token}'"))?
                    .to_string(),
            ),
        },
        _ => return Err(format!("invalid command token '{token}'")),
    };

    parsed.validate()?;
    Ok(parsed)
}

fn parse_duration(value: Option<&str>, token: &str) -> Result<u64, String> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("invalid command token '{token}'"))?
        .parse::<u64>()
        .map_err(|_| format!("invalid duration in command token '{token}'"))
}

fn require_no_value(token: &str, value: Option<&str>) -> Result<(), String> {
    match value {
        Some(_) => Err(format!("invalid command token '{token}'")),
        None => Ok(()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownDevice {
    pub base_url: String,
    pub api_key: String,
    pub cert_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert_pem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CliConfig {
    pub devices: BTreeMap<String, KnownDevice>,
}

impl CliConfig {
    pub fn path() -> Result<PathBuf, String> {
        let config_dir = dirs::config_dir().ok_or("failed to resolve ~/.config directory")?;
        Ok(config_dir.join("lilbug.json"))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|err| format!("failed to parse {}: {err}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }

        let body = serde_json::to_string_pretty(self)
            .map_err(|err| format!("failed to encode cli config: {err}"))?;
        fs::write(path, format!("{body}\n"))
            .map_err(|err| format!("failed to write {}: {err}", path.display()))
    }

    pub fn insert_device(&mut self, nickname: String, device: KnownDevice) {
        self.devices.insert(nickname, device);
    }

    pub fn rename_device(&mut self, old_nickname: &str, new_nickname: String) -> Result<(), String> {
        if old_nickname == new_nickname {
            return Ok(());
        }

        let device = self
            .devices
            .remove(old_nickname)
            .ok_or_else(|| format!("unknown device nickname '{old_nickname}'"))?;
        self.devices.insert(new_nickname, device);
        Ok(())
    }

    pub fn get_device(&self, nickname: &str) -> Result<&KnownDevice, String> {
        self.devices
            .get(nickname)
            .ok_or_else(|| format!("unknown device nickname '{nickname}'"))
    }
}

pub fn sha256_fingerprint(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("SHA256:{}", hex::encode_upper(digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rev1_command_tokens() {
        assert_eq!(
            parse_command_token("fwd:300").unwrap(),
            CommandRequest {
                command: "forward".to_string(),
                duration_ms: Some(300),
                value: None,
            }
        );
        assert_eq!(
            parse_command_token("face:happy").unwrap(),
            CommandRequest {
                command: "face".to_string(),
                duration_ms: None,
                value: Some("happy".to_string()),
            }
        );
    }

    #[test]
    fn rejects_invalid_command_tokens() {
        assert!(parse_command_token("fwd").is_err());
        assert!(parse_command_token("stop:100").is_err());
        assert!(parse_command_token("face:angry").is_err());
    }

    #[test]
    fn config_patch_updates_expected_fields() {
        let mut state = DeviceState::from_persisted(StartupMode::Wifi, None);
        state.apply_config_patch(&ConfigPatchRequest {
            nickname: Some("bug-01".to_string()),
            wifi_ssid: Some("lab-net".to_string()),
            wifi_password: Some("secret".to_string()),
            render_mode: Some(RenderMode::Local),
        });

        assert_eq!(state.config.nickname, "bug-01");
        assert_eq!(state.config.wifi.ssid, "lab-net");
        assert_eq!(state.config.wifi.password, "secret");
    }

    #[test]
    fn cli_config_round_trip_preserves_known_devices() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("lilbug.json");
        let mut config = CliConfig::default();
        config.insert_device(
            "anthony".to_string(),
            KnownDevice {
                base_url: DEFAULT_WIFI_URL.to_string(),
                api_key: "lb_test".to_string(),
                cert_fingerprint: "SHA256:ABC123".to_string(),
                cert_pem: Some("pem".to_string()),
            },
        );

        config.save(&path).unwrap();
        let loaded = CliConfig::load(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn rename_device_moves_record_to_new_nickname() {
        let mut config = CliConfig::default();
        config.insert_device(
            "anthony".to_string(),
            KnownDevice {
                base_url: DEFAULT_WIFI_URL.to_string(),
                api_key: "lb_test".to_string(),
                cert_fingerprint: "SHA256:ABC123".to_string(),
                cert_pem: Some("pem".to_string()),
            },
        );

        config.rename_device("anthony", "bug-02".to_string()).unwrap();

        assert!(config.get_device("anthony").is_err());
        assert_eq!(config.get_device("bug-02").unwrap().api_key, "lb_test");
    }
}

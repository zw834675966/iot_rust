//! Runtime configuration loaded from `config.toml` (or defaults if missing).

use anyhow::Context;
use serde::Deserialize;
use std::path::Path;

/// Top-level application configuration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub broker: Broker,
    pub modbus: Modbus,
    pub mqtt: Mqtt,
    pub db: Db,
}

impl Config {
    /// Load from `config.toml` next to the working directory; falls back to
    /// built-in defaults when the file is absent (anyhow-error on malformed TOML).
    pub fn load(path: &str) -> anyhow::Result<Self> {
        if !Path::new(path).exists() {
            tracing::warn!("config file '{path}' not found - using built-in defaults");
            return Ok(Config::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file '{path}'"))?;
        let cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("failed to parse config file '{path}'"))?;
        tracing::info!("loaded config from '{path}'");
        Ok(cfg)
    }
}

/// Embedded MQTT broker bind address.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Broker {
    pub host: String,
    pub port: u16,
}

impl Default for Broker {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 1883,
        }
    }
}

/// Modbus TCP endpoint (simulator + collector target).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Modbus {
    pub host: String,
    pub port: u16,
    pub register: u16,
    pub interval_ms: u64,
}

impl Default for Modbus {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5020,
            register: 0,
            interval_ms: 2000,
        }
    }
}

/// MQTT client settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Mqtt {
    pub client_id: String,
    pub topic: String,
    pub sub_topic: String,
}

impl Default for Mqtt {
    fn default() -> Self {
        Self {
            client_id: "iot-gateway-collector".to_string(),
            topic: "iot/site1/gateway1/data".to_string(),
            sub_topic: "iot/#".to_string(),
        }
    }
}

/// SQLite historian path.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Db {
    pub path: String,
}

impl Default for Db {
    fn default() -> Self {
        Self {
            path: "data/iot_gateway.db".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_documented_ports() {
        let cfg = Config::default();
        assert_eq!(cfg.broker.port, 1883);
        assert_eq!(cfg.modbus.port, 5020);
        assert_eq!(cfg.mqtt.topic, "iot/site1/gateway1/data");
    }

    #[test]
    fn parse_sample_toml() {
        let raw = r#"
            [broker]
            host = "10.0.0.1"
            port = 2883

            [modbus]
            port = 5502
            interval_ms = 500
        "#;
        let cfg: Config = toml::from_str(raw).expect("valid toml");
        assert_eq!(cfg.broker.host, "10.0.0.1");
        assert_eq!(cfg.broker.port, 2883);
        assert_eq!(cfg.modbus.port, 5502);
        assert_eq!(cfg.modbus.interval_ms, 500);
        // untouched fields fall back to defaults
        assert_eq!(cfg.db.path, "data/iot_gateway.db");
    }

    fn temp_config_path(name: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        std::env::temp_dir().join(format!("iot_gateway_cfg_{pid}_{name}.toml"))
    }

    #[test]
    fn load_reads_values_from_file() {
        let path = temp_config_path("load");
        let raw = r#"
            [broker]
            port = 2883
            [modbus]
            interval_ms = 500
        "#;
        std::fs::write(&path, raw).expect("write temp config");

        let cfg = Config::load(path.to_str().expect("utf-8 path")).expect("load config");
        assert_eq!(cfg.broker.port, 2883);
        assert_eq!(cfg.modbus.interval_ms, 500);
        // untouched fields fall back to defaults
        assert_eq!(cfg.broker.host, "127.0.0.1");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_falls_back_to_defaults() {
        let path = temp_config_path("does_not_exist");
        let _ = std::fs::remove_file(&path);
        let cfg = Config::load(path.to_str().expect("utf-8 path")).expect("load defaults");
        assert_eq!(cfg.broker.port, 1883);
        assert_eq!(cfg.modbus.port, 5020);
    }

    #[test]
    fn load_malformed_toml_returns_err() {
        let path = temp_config_path("bad");
        std::fs::write(&path, "[broker\nport = not a number").expect("write temp config");
        let result = Config::load(path.to_str().expect("utf-8 path"));
        assert!(result.is_err());
        let _ = std::fs::remove_file(&path);
    }
}

use anyhow::{Context, Result};
use rumqttc::{AsyncClient, MqttOptions, QoS};
use rumqttd::{Broker, Config, ConnectionSettings, RouterConfig, ServerSettings};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tracing::info;

/// Build minimal rumqttd config: MQTT v4 TCP listener on the configured
/// host/port (127.0.0.1:1883 by default), no auth.
///
/// `host` must be an IP literal (e.g. `127.0.0.1`); a hostname is rejected as
/// bad configuration rather than panicking, because rumqttd binds a
/// [`SocketAddr`] directly.
pub fn broker_config(host: &str, port: u16) -> Result<Config> {
    let mut v4 = HashMap::new();
    let listen = format!("{host}:{port}")
        .parse::<SocketAddr>()
        .with_context(|| {
            format!("invalid broker bind address '{host}:{port}' (expected IP:port)")
        })?;
    v4.insert(
        "tcp-1".to_string(),
        ServerSettings {
            name: "tcp-1".to_string(),
            listen,
            tls: None,
            next_connection_delay_ms: 0,
            connections: ConnectionSettings {
                connection_timeout_ms: 1000,
                max_payload_size: 268435465,
                max_inflight_count: 100,
                auth: None,
                external_auth: None,
                dynamic_filters: false,
            },
        },
    );

    Ok(Config {
        id: 0,
        router: RouterConfig {
            max_connections: 100,
            max_outgoing_packet_count: 200,
            max_segment_size: 524288000,
            max_segment_count: 10,
            custom_segment: None,
            initialized_filters: None,
            shared_subscriptions_strategy: Default::default(),
        },
        v4: Some(v4),
        v5: None,
        ws: None,
        cluster: None,
        console: None,
        bridge: None,
        prometheus: None,
        metrics: None,
    })
}

/// Start embedded MQTT broker (blocks - spawn on a dedicated thread)
pub fn start_broker(host: &str, port: u16) -> Result<()> {
    let cfg = broker_config(host, port)?;
    let mut broker = Broker::new(cfg);
    info!("MQTT broker starting on {host}:{port}");
    broker.start()?;
    Ok(())
}

/// Publish a JSON payload to a topic (AtMostOnce).
pub async fn publish_json(
    client: &AsyncClient,
    topic: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    let raw = serde_json::to_string(payload)?;
    client
        .publish(topic, QoS::AtMostOnce, false, raw.into_bytes())
        .await?;
    Ok(())
}

/// Create a rumqttc async client connected to the local broker
pub async fn create_client(client_id: &str, host: &str, port: u16) -> Result<AsyncClient> {
    let mut opts = MqttOptions::new(client_id, host, port);
    opts.set_keep_alive(Duration::from_secs(10));
    let (client, mut eventloop) = AsyncClient::new(opts, 10);

    // Drive the event loop in background
    tokio::spawn(async move {
        loop {
            if let Err(e) = eventloop.poll().await {
                tracing::warn!("MQTT eventloop error: {e}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    });

    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_config_accepts_ip_literal() {
        assert!(broker_config("127.0.0.1", 1883).is_ok());
    }

    #[test]
    fn broker_config_rejects_non_ip_host_without_panicking() {
        // A hostname is bad configuration for a direct SocketAddr bind; it must
        // surface as Err, not panic (regression guard for the former .expect()).
        assert!(broker_config("localhost", 1883).is_err());
        assert!(broker_config("not-an-ip", 1883).is_err());
    }
}

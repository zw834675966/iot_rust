mod config;
mod modbus_collector;
mod mqtt;

use iot_gateway::db;

use anyhow::Result;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("iot_gateway starting…");

    // 0) Load runtime configuration (config.toml, or built-in defaults)
    let cfg = config::Config::load("config.toml")?;

    // 1) Embedded MQTT broker
    let broker_host = cfg.broker.host.clone();
    let broker_port = cfg.broker.port;
    std::thread::spawn(move || {
        if let Err(e) = mqtt::start_broker(&broker_host, broker_port) {
            tracing::error!("MQTT broker error: {e}");
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 2) Mock Modbus TCP server (stand-in for real RS485 gateway)
    let modbus_addr = format!("{}:{}", cfg.modbus.host, cfg.modbus.port);
    tokio::spawn(async move {
        if let Err(e) = modbus_collector::start_simulator(&modbus_addr).await {
            tracing::error!("simulator error: {e}");
        }
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 3) SQLite historian
    let historian = Arc::new(db::Db::open(&cfg.db.path)?);

    // 4) MQTT client for publishing collected values
    let client =
        mqtt::create_client(&cfg.mqtt.client_id, &cfg.broker.host, cfg.broker.port).await?;

    // 5) Verification subscriber (simulates FUXA)
    let sub_host = cfg.broker.host.clone();
    let sub_topic = cfg.mqtt.sub_topic.clone();
    let (sub_client, mut sub_eventloop) = {
        let mut opts = MqttOptions::new("verification-sub", &sub_host, cfg.broker.port);
        opts.set_keep_alive(Duration::from_secs(10));
        AsyncClient::new(opts, 10)
    };
    sub_client.subscribe(&sub_topic, QoS::AtMostOnce).await?;
    tokio::spawn(async move {
        loop {
            match sub_eventloop.poll().await {
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(p))) => {
                    info!(
                        "📥 FUXA would receive → {}: {}",
                        p.topic,
                        String::from_utf8_lossy(&p.payload)
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("subscriber error: {e}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    // 6) Modbus collector → MQTT publish + SQLite insert
    let publish_client = client.clone();
    let db = Arc::clone(&historian);
    let topic = cfg.mqtt.topic.clone();
    let interval = Duration::from_millis(cfg.modbus.interval_ms);
    modbus_collector::run_collector(
        &cfg.modbus.host,
        cfg.modbus.port,
        cfg.modbus.register,
        interval,
        move |value| {
            // FUXA MQTT driver does `JSON.parse(payload)[memaddress]` (single-level key),
            // so the payload must be a flat JSON object — one top-level key per tag.
            let ts = chrono::Utc::now().timestamp_millis();
            let payload = serde_json::json!({
                "ts": ts,
                "register1": value,
                "quality": "good"
            });

            // Persist to SQLite
            if let Err(e) = db.insert("register1", value as f64, ts, "good") {
                tracing::error!("db insert error: {e}");
            }

            // Publish to MQTT
            let client = publish_client.clone();
            let topic = topic.clone();
            tokio::spawn(async move {
                if let Err(e) = mqtt::publish_json(&client, &topic, &payload).await {
                    tracing::error!("publish error: {e}");
                }
            });
        },
    )
    .await?;

    Ok(())
}

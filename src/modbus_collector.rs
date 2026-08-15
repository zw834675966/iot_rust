use anyhow::Result;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_modbus::prelude::*;
use tokio_modbus::server::tcp::Server;
use tokio_modbus::server::Service;
use tracing::info;

// ── Simulator (mock Modbus TCP gateway) ──────────────────────────────

/// Starts a mock Modbus TCP server on 127.0.0.1:5020.
/// Holding register 0 returns an incrementing counter on each read.
pub async fn start_simulator(addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let server = Server::new(listener);
    info!("Modbus TCP simulator listening on {addr}");

    // Shared counter across all connections
    let counter = Arc::new(AtomicU16::new(1000));

    let on_connected = move |_stream, _socket_addr| {
        let counter = Arc::clone(&counter);
        async move {
            let svc = Arc::new(MockService { counter });
            Ok(Some((svc, _stream)))
        }
    };
    let on_process_error = |err| tracing::error!("modbus server error: {err}");
    server.serve(&on_connected, on_process_error).await?;
    Ok(())
}

struct MockService {
    counter: Arc<AtomicU16>,
}

impl Service for MockService {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = std::future::Ready<Result<Self::Response, Self::Exception>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let result = match req {
            Request::ReadHoldingRegisters(addr, cnt) => {
                if addr == 0 && cnt == 1 {
                    let val = self.counter.fetch_add(1, Ordering::SeqCst);
                    info!("[simulator] read register 0 → {val}");
                    Ok(Response::ReadHoldingRegisters(vec![val]))
                } else {
                    Err(ExceptionCode::IllegalDataAddress)
                }
            }
            _ => Err(ExceptionCode::IllegalFunction),
        };
        std::future::ready(result)
    }
}

// ── Collector ────────────────────────────────────────────────────────

/// Connect to a Modbus TCP gateway and periodically read one holding register.
/// Calls `on_value(value)` for each successful read.
pub async fn run_collector<F>(
    host: &str,
    port: u16,
    register_addr: u16,
    interval: Duration,
    mut on_value: F,
) -> Result<()>
where
    F: FnMut(u16) + Send + 'static,
{
    let addr = format!("{host}:{port}");
    info!("Modbus collector → {addr}, register={register_addr}, interval={interval:?}");

    let mut ctx = tcp::connect(addr.parse()?).await?;

    loop {
        match ctx.read_holding_registers(register_addr, 1).await {
            Ok(Ok(regs)) => {
                if let Some(&value) = regs.first() {
                    on_value(value);
                }
            }
            Ok(Err(e)) => tracing::warn!("Modbus exception: {e:?}"),
            Err(e) => tracing::error!("Modbus read error: {e}"),
        }
        tokio::time::sleep(interval).await;
    }
}

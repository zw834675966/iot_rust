use iot_gateway::db::Db;
fn main() -> anyhow::Result<()> {
    let db = Db::open("data/iot_gateway.db")?;
    let rows = db.recent("register1", 10)?;
    println!("=== tag_history (latest 10) ===");
    for (ts, val, q) in &rows {
        println!("  ts={ts}  value={val}  quality={q}");
    }
    println!("total: {}", rows.len());
    Ok(())
}

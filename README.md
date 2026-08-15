# iot_rust · IoT Gateway（Rust）

Rust 编写的 Modbus TCP 采集网关 MVP：**Modbus TCP 采集 → JSON → 本地嵌入式 MQTT → FUXA 订阅展示 + SQLite 历史库**。

当前内置 Modbus TCP **模拟器**（代替真实 RS485/Modbus 网关），数据链路完整可跑：`cargo run` 即启动整条管线，配合外部 FUXA（HMI 组态软件）可实时看到数据刷新与 DAQ 历史落盘。

## 架构

```
Modbus TCP simulator (127.0.0.1:5020, 递增计数器)
  → iot_gateway (Rust collector, 2s 周期, register 0)
    → MQTT broker (rumqttd 嵌入式, 127.0.0.1:1883)
      → topic: iot/site1/gateway1/data   (扁平 JSON)
        → FUXA (Web http://127.0.0.1:1881)
        → 内置验证订阅者（模拟 FUXA 接收，日志回显）
      → SQLite 历史 (data/iot_gateway.db, tag_history 表)
```

- **采集端**：`tokio-modbus` TCP 客户端周期读取 holding register（默认 2s 一次）
- **模拟器**：`tokio-modbus` TCP 服务端，register 0 每次读返回递增计数器
- **消息**：嵌入式 `rumqttd` broker + `rumqttc` 客户端，payload 为**扁平 JSON**（`{"ts","register1","quality"}`，单层 key——对齐 FUXA MQTT 驱动的 `JSON.parse(payload)[memaddress]` 取值方式，不支持嵌套）
- **历史**：`rusqlite`（bundled）SQLite 库，WAL 模式 + (tag_name, timestamp) 索引

## 快速开始

前置：Rust stable 工具链（`rust-toolchain.toml` 已固定）。

```bash
# 1. 启动整条链路（broker + 模拟器 + 采集 + SQLite + 验证订阅者）
cargo run

# 2. 另开终端查历史（最新 10 条）
cargo run --example query_db
```

运行日志中可见完整链路：

```text
INFO iot_gateway::modbus_collector: [simulator] read register 0 → 1151
INFO iot_gateway: 📥 FUXA would receive → iot/site1/gateway1/data: {"quality":"good","register1":1151,"ts":1786760256413}
```

## 配置（config.toml）

`cargo run` 时自动加载；文件缺失时回退内置默认值。

| 段 | 键 | 默认 | 说明 |
|---|---|---|---|
| `[broker]` | `host` / `port` | `127.0.0.1` / `1883` | 嵌入式 MQTT broker 绑定地址（须为 IP 字面量） |
| `[modbus]` | `host` | `127.0.0.1` | 采集目标（模拟器）地址 |
| `[modbus]` | `port` | `5020` | 模拟器端口 |
| `[modbus]` | `register` | `0` | 采集的 holding register 地址 |
| `[modbus]` | `interval_ms` | `2000` | 采集周期 |
| `[mqtt]` | `client_id` | `iot-gateway-collector` | 发布客户端 ID |
| `[mqtt]` | `topic` | `iot/site1/gateway1/data` | 发布主题 |
| `[mqtt]` | `sub_topic` | `iot/#` | 内置验证订阅者主题 |
| `[db]` | `path` | `data/iot_gateway.db` | SQLite 历史库路径 |

## 常用命令

```bash
cargo run                 # 启动网关（完整链路）
cargo run --example query_db   # 查询最近历史
cargo test --all-targets  # 单元测试（8 个）
cargo fmt-check           # 格式检查
cargo lint                # clippy --all-targets -D warnings
cargo fmt --all           # 格式化
```

## FUXA 对接（可选，外部组件）

FUXA 不在本仓库内（vendored 副本见 `fuxa/`），本机完整验证路径记录在 `fuxa-project/README.md`。

```bash
# FUXA（本机外部安装）
cd D:/FUXA/FUXA/server && node main.js
# 浏览器打开
http://127.0.0.1:1881/
```

- **设备** `d_iot_gateway`：type=`MQTTclient`，address=`mqtt://127.0.0.1:1883`
- **Tag** `t_register1`：address=`iot/site1/gateway1/data`，type=`json`，memaddress=`register1`，options.subs=`["subs"]`，daq.enabled=true
- **落盘点**：`D:/FUXA/FUXA/server/_appdata/project.fuxap.db`（devices 表）+ DAQ 归档 `_db/daq-data_d_iot_gateway_*.db`
- 已验证：FUXA 连接成功、数据 2s 周期刷新、DAQ 持续落盘（1159→1160→1161，与采集计数器吻合）

## 项目结构

```text
src/
  main.rs               # 仅装配：broker / 模拟器 / 采集 / SQLite / 订阅者
  modbus_collector.rs   # TCP 模拟器（MockService）+ 周期采集器
  mqtt.rs               # rumqttd broker 配置/启动 + rumqttc 发布/客户端
  db.rs                 # SQLite 历史库（tag_history）
  config.rs             # config.toml 运行时配置（内置默认值）
  lib.rs                # pub mod db，供 examples 复用
examples/
  query_db.rs           # 历史查询工具
config.toml             # 统一配置（IP/端口/主题集中管理）
data/                   # SQLite 库与运行日志（git 忽略）
```

## 已知遗留（不阻塞 MVP 验收）

- FUXA HMI 页面（views）未创建：可在 FUXA UI 中新建 View + Value/Gauge 组件绑定 `register1` tag 查看实时刷新
- FUXA DAQ 归档偶发 EBUSY 噪音：旧启动残留 `_db/daq-data_d_iot_gateway_*.db` 文件锁，属归档重命名失败，不影响采集
- 内置验证订阅者只是模拟 FUXA 接收；真实 FUXA 与它同连一个 broker

## 工程边界

- Broker 与模拟器绑定 **127.0.0.1**（除非显式修改配置）
- 无 `unsafe`；I/O/协议路径禁用 `.unwrap()`（`clippy.toml` 强制）
- 日志走 `tracing`，生产路径无 `println!` / `dbg!`

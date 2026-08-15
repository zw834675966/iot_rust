# FUXA MQTT 端到端验证记录

## 拓扑

```
Modbus TCP simulator (127.0.0.1:5020, 递增计数器)
  → iot_gateway (Rust collector, 2s 周期)
    → MQTT broker (rumqttd 127.0.0.1:1883)
      → topic: iot/site1/gateway1/data
      → FUXA (D:/FUXA/FUXA, Web http://127.0.0.1:1881)
        → tag register1 (type=json, memaddress=register1)
        → DAQ 落盘 _db/daq-data_d_iot_gateway_*.db
```

## Payload 格式（对齐依据）

FUXA MQTT 驱动：`server/runtime/devices/mqtt/index.js`

```js
// message 回调（订阅 tag.address 后）
if (data.tags[id].type === 'json' && data.tags[id].options && data.tags[id].options.subs && data.tags[id].memaddress) {
    var subitems = JSON.parse(data.tags[id].rawValue);
    if (!utils.isNullOrUndefined(subitems[data.tags[id].memaddress])) {
        data.tags[id].rawValue = subitems[data.tags[id].memaddress];
    }
}
```

→ **单层 key 取值**，不支持嵌套。Rust 发布扁平 JSON：

```json
{"quality":"good","register1":1151,"ts":1786760256413}
```

（`src/main.rs` 已对齐，注释说明缘由）

## FUXA 配置落盘点

- 库：`D:/FUXA/FUXA/server/_appdata/project.fuxap.db`（devices 表）
- 设备 `d_iot_gateway`：type=`MQTTclient`，property.address=`mqtt://127.0.0.1:1883`
- tag `t_register1`：address=`iot/site1/gateway1/data`，type=`json`，memaddress=`register1`，options.subs=`["subs"]`，daq.enabled=true

## FUXA 本地修复（v1.2.7-2516 → 对齐官方 master）

1. `server/runtime/project/index.js`：新增 `getServer()`（返回 `data.devices['0'] || data.server || null`）并导出
   —— 本地旧版缺失，FuxaServer 设备从不加载
2. `server/runtime/devices/index.js` `load()`：加载 serverDevice（+ 跳过 id===FuxaServerId 重复）
   —— 修复 `Cannot read properties of undefined (reading 'setDeviceConnectionStatus')`
3. `server/runtime/devices/index.js` `setDeviceConnectionStatus()`：加存在性防御
4. server 设备行 value 补 `"tags": {}`
   —— 修复 `Object.keys(data.tags)` 对 undefined 崩溃（官方 master 有 `data.tags || {}` 防御）

备份：`*.bak`（同目录）

## 验证证据

- `command:` Rust 日志：`published → iot/site1/gateway1/data: {"quality":"good","register1":1151,...}` + `📥 FUXA would receive` 回显
- `command:` FUXA 日志：`'IoT Gateway' connected!` + `FUXA started!` + WebServer 1881
- `command:` DAQ 落盘：`_db/daq-data_d_iot_gateway_*.db` data 表持续写入递增值（1159→1160→1161，2s 间隔，与发布计数器吻合）
- `command:` daq-map：`(1, 't_register1', 'register1', 'json')` 确认 tag 映射

## 启动方式

```bash
# Rust 网关
cd D:/rust/iot_rust && cargo run
# FUXA（本机）
cd D:/FUXA/FUXA/server && node main.js
# 浏览器
http://127.0.0.1:1881/
```

## 遗留事项（不阻塞验收）

- FUXA DAQ 归档 EBUSY 噪音：旧启动残留 `_db/daq-data_d_iot_gateway_*.db` 文件锁，属归档重命名失败，不影响采集
- HMI 页面（views）未创建：可在 FUXA UI 中新建 View + Value/Gauge 组件绑定 register1 tag 查看实时刷新
- 验证 subscriber 只是模拟 FUXA 接收，真实 FUXA 已连接同一 broker

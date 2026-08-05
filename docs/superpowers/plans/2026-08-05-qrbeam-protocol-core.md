# QRBeam 协议核心第一阶段实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现一个可在 Rust 原生环境验证的 QRBeam 协议闭环：文件清单和帧具有稳定线格式，512 KiB 分段独立使用 RaptorQ，在丢帧、重复和乱序后恢复原始文件并输出区块状态。

**架构：** 第一阶段只实现无 I/O 的 `qrbeam-core` Rust 库，不接二维码、相机和界面。发送侧把清单和每个区块编码成带 CRC32C 的帧；接收侧按会话、区块和编码符号 ID 去重，使用 RaptorQ 恢复区块，再以区块 CRC32C 和文件 BLAKE3 验证完成。确定性时间轴保存帧元数据，使后续 Web 和 CLI 可以定位、后退和回补。

**技术栈：** Rust 1.97.1、Cargo workspace、`raptorq = 2.0.1`、`blake3 = 1.8.5`、`crc32c = 0.6.8`、`thiserror = 2.0.19`。

---

## 范围边界

本计划交付协议核心的第一个可工作里程碑。QR 编码、Web/PWA、Linux CLI、Flutter App、文件压缩、磁盘持久化和相机性能自检分别使用后续计划实现。第一阶段保留这些组件所需的稳定 API 和线协议字段，但不提前实现界面层。

## 文件结构

| 文件 | 职责 |
|---|---|
| `Cargo.toml` | Rust workspace、统一依赖版本和 lint 配置 |
| `rust-toolchain.toml` | 锁定本阶段验证过的 Rust 1.97.1 minimal 工具链 |
| `crates/qrbeam-core/Cargo.toml` | 协议核心 crate 元数据和依赖 |
| `crates/qrbeam-core/src/lib.rs` | 对外导出稳定协议 API |
| `crates/qrbeam-core/src/constants.rs` | 文件、区块、符号和帧上限 |
| `crates/qrbeam-core/src/error.rs` | 所有可恢复解析和接收错误 |
| `crates/qrbeam-core/src/frame.rs` | 56 字节帧头、载荷、CRC32C 和黄金字节编码 |
| `crates/qrbeam-core/src/manifest.rs` | 文件清单、档位、确定性二进制编码和清单分片 |
| `crates/qrbeam-core/src/segment.rs` | 单区块 RaptorQ 编码、去重、恢复和 CRC 校验 |
| `crates/qrbeam-core/src/timeline.rs` | 全局帧时间轴、历史定位和指定区块回补 |
| `crates/qrbeam-core/src/session.rs` | 多区块发送和接收会话、区块图状态及最终哈希 |
| `crates/qrbeam-core/tests/frame_wire.rs` | 帧黄金向量、损坏和边界测试 |
| `crates/qrbeam-core/tests/manifest_wire.rs` | 清单往返、分片、损坏和大小上限测试 |
| `crates/qrbeam-core/tests/segment_recovery.rs` | RaptorQ 系统符号、修复符号、重复和乱序测试 |
| `crates/qrbeam-core/tests/timeline.rs` | 帧号稳定性、后退 100 帧、区块定位和回补测试 |
| `crates/qrbeam-core/tests/end_to_end.rs` | 50% 丢帧、重复、乱序和损坏下的文件恢复测试 |
| `docs/protocol/wire-format-v1.md` | 跨 Rust、WASM、iOS 和 Android 共用的线格式定义 |

## 固定协议决定

- 帧使用小端整数。
- 固定帧头为 56 字节；最大 QR 二进制帧为 2953 字节。
- CRC32C 覆盖“CRC 字段清零后的完整帧头 + payload”。
- 数据和修复帧的 payload 是连续 256 字节 RaptorQ 编码符号；`first_symbol_id` 和 `symbol_count` 标识范围。
- 每个 512 KiB 文件区块是独立 RaptorQ 对象，`source_block_number` 固定为 0；QRBeam 的 `segment_index` 负责文件级分块。
- 每个区块使用 `ObjectTransmissionInformation::new(actual_length, 256, 1, 1, 1)`。
- 清单分片使用 `FrameType::Manifest`；`segment_index` 保存分片序号，`first_symbol_id` 保存分片总数。
- 帧头偏移固定为：magic `0..4`、version `4`、type `5`、flags `6`、channel `7`、profile `8`、reserved `9`、header length `10..12`、session `12..28`、file ID `28..32`、global index `32..40`、segment `40..44`、first symbol `44..48`、symbol count `48..50`、payload length `50..52`、CRC32C `52..56`。

### 任务 1：建立可复现的 Rust workspace

**文件：**
- 创建：`Cargo.toml`
- 创建：`rust-toolchain.toml`
- 创建：`crates/qrbeam-core/Cargo.toml`
- 创建：`crates/qrbeam-core/src/lib.rs`
- 创建：`crates/qrbeam-core/src/constants.rs`
- 修改：`docs/superpowers/specs/2026-08-04-qrbeam-optical-transfer-design.md`

- [x] **步骤 1：添加 workspace 和锁定工具链**

`Cargo.toml` 使用以下完整配置：

```toml
[workspace]
members = ["crates/qrbeam-core"]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.97"

[workspace.dependencies]
blake3 = "=1.8.5"
crc32c = "=0.6.8"
raptorq = "=2.0.1"
thiserror = "=2.0.19"

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
```

`rust-toolchain.toml`：

```toml
[toolchain]
channel = "1.97.1"
profile = "minimal"
components = ["clippy", "rustfmt"]
```

- [x] **步骤 2：创建空协议 crate 和常量**

`crates/qrbeam-core/Cargo.toml`：

```toml
[package]
name = "qrbeam-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
blake3.workspace = true
crc32c.workspace = true
raptorq.workspace = true
thiserror.workspace = true

[lints]
workspace = true
```

`src/constants.rs` 定义：

```rust
pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_FILE_BYTES: u64 = 100_000_000;
pub const SEGMENT_BYTES: usize = 512 * 1024;
pub const SYMBOL_BYTES: usize = 256;
pub const FRAME_HEADER_BYTES: usize = 56;
pub const MAX_QR_FRAME_BYTES: usize = 2_953;
pub const MAX_FRAME_PAYLOAD_BYTES: usize = MAX_QR_FRAME_BYTES - FRAME_HEADER_BYTES;
pub const MANIFEST_FRAGMENT_DATA_BYTES: usize = SYMBOL_BYTES - 32;
```

`src/lib.rs` 只导出 `constants`，不实现后续行为。

- [x] **步骤 3：修正文档中的项目目录**

把批准规格中的旧目录 `/Users/blackhook/ai/openclaw/qrbeam/` 改成 `/Users/blackhook/ai/qrbeam/`。

- [x] **步骤 4：运行格式、检查和空测试基线**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo fmt --all --check
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
/Users/blackhook/.cargo/bin/cargo test --workspace
```

预期：三个命令退出码均为 0，测试报告为 0 failed。

- [x] **步骤 5：Commit**

```bash
git add Cargo.toml rust-toolchain.toml Cargo.lock crates docs/superpowers/specs/2026-08-04-qrbeam-optical-transfer-design.md
git commit -m "chore: 初始化 QRBeam Rust 协议工作区"
```

### 任务 2：实现具有 CRC32C 的固定帧线格式

**文件：**
- 创建：`crates/qrbeam-core/src/error.rs`
- 创建：`crates/qrbeam-core/src/frame.rs`
- 创建：`crates/qrbeam-core/tests/frame_wire.rs`
- 修改：`crates/qrbeam-core/src/lib.rs`

- [x] **步骤 1：编写失败的帧往返和黄金字节测试**

测试构造 `FrameHeader`：session 为 `[0x11; 16]`、file ID `0x0102_0304`、全局帧 `0x0102_0304_0506_0708`、区块 9、首符号 10、符号数 1、payload 为 256 个 `0xA5`。断言：

```rust
let bytes = frame.encode().unwrap();
assert_eq!(&bytes[0..4], b"QRBM");
assert_eq!(bytes.len(), FRAME_HEADER_BYTES + SYMBOL_BYTES);
assert_eq!(u16::from_le_bytes(bytes[10..12].try_into().unwrap()), 56);
assert_eq!(Frame::decode(&bytes).unwrap(), frame);
```

另写三个独立测试：翻转 payload 一位返回 `ProtocolError::CrcMismatch`；未知 frame type 返回 `ProtocolError::UnknownFrameType`；超过 `MAX_FRAME_PAYLOAD_BYTES` 返回 `ProtocolError::PayloadTooLarge`。

- [x] **步骤 2：运行测试验证红灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test frame_wire
```

预期：FAIL，原因是 `frame` 模块和公开类型尚不存在。

- [x] **步骤 3：实现最小帧编码和解析**

公开 API 固定为：

```rust
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameType { Manifest = 1, Test = 2, Data = 3, Repair = 4, Control = 5 }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    pub frame_type: FrameType,
    pub flags: u8,
    pub channel_id: u8,
    pub profile_id: u8,
    pub session_id: [u8; 16],
    pub file_id: u32,
    pub global_frame_index: u64,
    pub segment_index: u32,
    pub first_symbol_id: u32,
    pub symbol_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame { pub header: FrameHeader, pub payload: Vec<u8> }

impl Frame {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError>;
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError>;
}
```

`encode` 验证 payload 上限、`u16` 长度、符号帧的 `payload.len() == symbol_count * SYMBOL_BYTES` 和 24 位 RaptorQ ESI 上限。`decode` 在任何切片前验证最小长度、magic、版本、header length、payload length和 CRC，不对不可信输入执行 panic。

- [x] **步骤 4：运行帧测试和全量检查验证绿灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test frame_wire
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
```

预期：帧测试全部通过，Clippy 0 error。

- [x] **步骤 5：Commit**

```bash
git add crates/qrbeam-core/src crates/qrbeam-core/tests/frame_wire.rs
git commit -m "feat: 定义 QRBeam 帧线格式"
```

### 任务 3：实现确定性文件清单和分片

**文件：**
- 创建：`crates/qrbeam-core/src/manifest.rs`
- 创建：`crates/qrbeam-core/tests/manifest_wire.rs`
- 修改：`crates/qrbeam-core/src/lib.rs`

- [x] **步骤 1：编写失败的清单测试**

测试覆盖：普通清单 encode/decode 完全相等；191 个区块 CRC 可以拆成多个 256 字节分片并乱序重组；文件大小 `100_000_001` 被拒绝；重复分片不增加完成度；缺一片时不能返回清单；翻转清单内容后完整 BLAKE3 不匹配。

公开测试数据使用：

```rust
let manifest = Manifest {
    session_id: [0x22; 16],
    file_id: 7,
    filename: "example.bin".into(),
    mime_type: "application/octet-stream".into(),
    original_length: 1_000_000,
    container_length: 1_000_000,
    compression: Compression::None,
    file_hash: [0x33; 32],
    segment_size: SEGMENT_BYTES as u32,
    symbol_size: SYMBOL_BYTES as u16,
    last_segment_length: 475_712,
    encoding_seed: [0x44; 16],
    profiles: Profile::defaults().to_vec(),
    segment_crc32c: vec![0x1234_5678, 0x90ab_cdef],
};
```

- [x] **步骤 2：运行测试验证红灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test manifest_wire
```

预期：FAIL，原因是 `manifest` 模块不存在。

- [x] **步骤 3：实现清单线格式和分片重组器**

公开 API 固定为：

```rust
pub enum Compression { None = 0, Gzip = 1 }
pub enum EccLevel { L = 0, M = 1 }
pub struct Profile {
    pub id: u8,
    pub symbols_per_frame: u8,
    pub ecc: EccLevel,
    pub target_fps: u8,
    pub min_module_pixels: u8,
    pub max_qr_version: u8,
}
pub struct Manifest {
    pub session_id: [u8; 16],
    pub file_id: u32,
    pub filename: String,
    pub mime_type: String,
    pub original_length: u64,
    pub container_length: u64,
    pub compression: Compression,
    pub file_hash: [u8; 32],
    pub segment_size: u32,
    pub symbol_size: u16,
    pub last_segment_length: u32,
    pub encoding_seed: [u8; 16],
    pub profiles: Vec<Profile>,
    pub segment_crc32c: Vec<u32>,
}
pub struct ManifestFragment {
    pub index: u32,
    pub count: u32,
    pub manifest_hash: [u8; 32],
    pub payload: Vec<u8>,
}
pub struct ManifestAssembler {
    manifest_hash: Option<[u8; 32]>,
    fragment_count: Option<u32>,
    fragments: Vec<Option<Vec<u8>>>,
    received: u32,
}

impl Profile {
    pub const fn defaults() -> [Self; 4];
}
impl Manifest {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError>;
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError>;
    pub fn fragments(&self) -> Result<Vec<ManifestFragment>, ProtocolError>;
}
impl ManifestFragment {
    pub fn into_frame(
        self,
        session_id: [u8; 16],
        file_id: u32,
        global_frame_index: u64,
        channel_id: u8,
        profile_id: u8,
    ) -> Result<Frame, ProtocolError>;
}
impl ManifestAssembler {
    pub fn new() -> Self;
    pub fn push(&mut self, frame: &Frame) -> Result<Option<Manifest>, ProtocolError>;
}
```

每个清单帧 payload 前 32 字节保存完整清单 BLAKE3，后面最多保存 224 字节分片数据，使整个 payload 不超过一个 256 字节稳定档符号。字符串使用 UTF-8 和 `u16` 长度，profile 固定 8 字节，区块 CRC 使用 `u32` 小端，清单末尾保存 CRC32C。解析时验证区块数量等于 `ceil(container_length / segment_size)`，最后区块长度正确，默认分段和符号大小符合协议，所有计数在分配内存前通过上限检查。

- [x] **步骤 4：运行清单测试和全量检查验证绿灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test manifest_wire
/Users/blackhook/.cargo/bin/cargo test --workspace
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
```

预期：全部测试通过，Clippy 0 error。

- [x] **步骤 5：Commit**

```bash
git add crates/qrbeam-core/src crates/qrbeam-core/tests/manifest_wire.rs
git commit -m "feat: 添加文件清单和分片协议"
```

### 任务 4：实现独立区块 RaptorQ 恢复

**文件：**
- 创建：`crates/qrbeam-core/src/segment.rs`
- 创建：`crates/qrbeam-core/tests/segment_recovery.rs`
- 修改：`crates/qrbeam-core/src/lib.rs`

- [x] **步骤 1：编写失败的区块恢复测试**

使用确定性数据 `(0..500_123).map(|i| (i % 251) as u8)`。测试分别验证：完整系统符号直接恢复；丢弃 50% 系统符号后加入足量修复符号恢复；符号乱序仍恢复；重复 ESI 返回 `SegmentUpdate::Duplicate`；载荷损坏但帧 CRC 被重新计算时最终区块 CRC 拒绝完成；最后不足 256 字节的区块按实际长度还原。

单个测试的核心过程：

```rust
let encoder = SegmentEncoder::new(3, &data).unwrap();
let mut decoder = SegmentDecoder::new(3, data.len(), crc32c::crc32c(&data)).unwrap();
for packet in encoder.source_packets().into_iter().step_by(2) {
    decoder.push(packet).unwrap();
}
for packet in encoder.repair_packets(0, encoder.source_symbol_count() / 2 + 64).unwrap() {
    if let SegmentUpdate::Complete(restored) = decoder.push(packet).unwrap() {
        assert_eq!(restored, data);
        return;
    }
}
panic!("repair symbols did not complete the segment");
```

- [x] **步骤 2：运行测试验证红灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test segment_recovery
```

预期：FAIL，原因是区块编码器和解码器不存在。

- [x] **步骤 3：实现 RaptorQ 包装层**

公开 API 固定为：

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolPacket { pub esi: u32, pub data: Vec<u8> }

pub struct SegmentEncoder {
    index: u32,
    actual_length: usize,
    source_symbol_count: u32,
    encoder: raptorq::Encoder,
}
impl SegmentEncoder {
    pub fn new(index: u32, data: &[u8]) -> Result<Self, ProtocolError>;
    pub fn source_symbol_count(&self) -> u32;
    pub fn source_packets(&self) -> Vec<SymbolPacket>;
    pub fn repair_packets(&self, start: u32, count: u32) -> Result<Vec<SymbolPacket>, ProtocolError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SegmentUpdate { Accepted { unique_symbols: u32 }, Duplicate, Complete(Vec<u8>) }

pub struct SegmentDecoder {
    index: u32,
    actual_length: usize,
    expected_crc32c: u32,
    seen_esi: std::collections::HashSet<u32>,
    decoder: raptorq::Decoder,
    completed: Option<Vec<u8>>,
}
impl SegmentDecoder {
    pub fn new(index: u32, actual_length: usize, expected_crc32c: u32) -> Result<Self, ProtocolError>;
    pub fn push(&mut self, packet: SymbolPacket) -> Result<SegmentUpdate, ProtocolError>;
}
```

每个区块创建一个 `raptorq::SourceBlockEncoder`/`SourceBlockDecoder`，RaptorQ source block ID 固定为 0。进入库前验证 ESI 小于 `1 << 24`、payload 恰好为 256 字节、区块长度不超过 512 KiB。RaptorQ 返回结果后截断到实际长度并验证 CRC32C；失败返回错误且不得把区块标记为完成。

- [x] **步骤 4：运行区块测试和全量检查验证绿灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test segment_recovery
/Users/blackhook/.cargo/bin/cargo test --workspace
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
```

预期：全部测试通过，Clippy 0 error。

- [x] **步骤 5：Commit**

```bash
git add crates/qrbeam-core/src crates/qrbeam-core/tests/segment_recovery.rs
git commit -m "feat: 实现分段 RaptorQ 恢复"
```

### 任务 5：实现可定位的确定性时间轴

**文件：**
- 创建：`crates/qrbeam-core/src/timeline.rs`
- 创建：`crates/qrbeam-core/tests/timeline.rs`
- 修改：`crates/qrbeam-core/src/lib.rs`

- [x] **步骤 1：编写失败的时间轴测试**

测试验证：全局帧号从 0 单调增加；同一显示 tick 的多通道计划共享全局帧号但通道不同；不同通道分配不同 ESI；`seek_back(100)` 饱和到 0；`seek_segment(7)` 返回最近包含区块 7 的历史帧；进入回补模式后只生成指定区块的新 repair ESI；退出回补后继续原时间轴；档位变化不改变 session ID 和 file ID。

时间轴测试使用：

```rust
let mut timeline = Timeline::new([0x55; 16], 9, vec![4, 4, 2]).unwrap();
let plans = timeline.next_tick(&[
    ChannelRequest { channel_id: 0, profile_id: 1, symbols_per_frame: 1 },
    ChannelRequest { channel_id: 1, profile_id: 2, symbols_per_frame: 2 },
]).unwrap();
assert_eq!(plans[0].global_frame_index, plans[1].global_frame_index);
assert_ne!(plans[0].first_symbol_id, plans[1].first_symbol_id);
```

- [x] **步骤 2：运行测试验证红灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test timeline
```

预期：FAIL，原因是 `timeline` 模块不存在。

- [x] **步骤 3：实现历史元数据和回补状态机**

公开 API 固定为：

```rust
pub struct ChannelRequest { pub channel_id: u8, pub profile_id: u8, pub symbols_per_frame: u16 }
pub enum SymbolKind { Source, Repair }
pub struct FramePlan {
    pub session_id: [u8; 16],
    pub file_id: u32,
    pub channel_id: u8,
    pub profile_id: u8,
    pub global_frame_index: u64,
    pub segment_index: u32,
    pub first_symbol_id: u32,
    pub symbol_count: u16,
    pub kind: SymbolKind,
}
pub struct Timeline {
    session_id: [u8; 16],
    file_id: u32,
    source_symbol_counts: Vec<u32>,
    source_cursor: Vec<u32>,
    repair_cursor: Vec<u32>,
    next_source_segment: usize,
    next_repair_segment: usize,
    repair_only_segment: Option<u32>,
    history: Vec<Vec<FramePlan>>,
    playhead: usize,
}
```

提供 `new`、`next_tick`、`seek_frame`、`seek_back`、`seek_forward`、`seek_segment`、`start_repair`、`stop_repair` 和 `current_frame`。历史只保存 `FramePlan`，不保存二维码图片或 payload。新计划先遍历全部区块的系统符号，再按区块轮询生成从未使用过的 repair ESI。

- [x] **步骤 4：运行时间轴测试和全量检查验证绿灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test timeline
/Users/blackhook/.cargo/bin/cargo test --workspace
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
```

预期：全部测试通过，Clippy 0 error。

- [x] **步骤 5：Commit**

```bash
git add crates/qrbeam-core/src crates/qrbeam-core/tests/timeline.rs
git commit -m "feat: 添加可寻址发送时间轴"
```

### 任务 6：完成文件级发送、接收和损坏信道闭环

**文件：**
- 创建：`crates/qrbeam-core/src/session.rs`
- 创建：`crates/qrbeam-core/tests/end_to_end.rs`
- 修改：`crates/qrbeam-core/src/lib.rs`

- [x] **步骤 1：编写失败的端到端测试**

生成 `1_200_123` 字节确定性文件，使其跨越 3 个区块。发送端持续生成系统帧和修复帧；release 集成测试信道按固定 xorshift64 种子执行 50% 丢帧、每第 11 帧重复、每 17 帧翻转一位并把批次逆序。接收端必须丢弃 CRC 错误、忽略重复、接受乱序，并最终逐字节还原文件。日常 debug 测试使用 64 KiB 文件验证相同闭环；另测未知会话被拒绝、文件 BLAKE3 不匹配时不返回 `Complete`。

核心断言：

```rust
let sender = SendSession::new("sample.bin", "application/octet-stream", &data, [7; 16], 42).unwrap();
let manifest = sender.manifest().clone();
let mut receiver = ReceiveSession::new(manifest).unwrap();
for frame_bytes in damaged_reordered_stream(&sender, 0x1234_5678_9abc_def0) {
    if let Ok(ReceiveUpdate::Complete(restored)) = receiver.ingest(&frame_bytes) {
        assert_eq!(restored, data);
        return;
    }
}
panic!("receiver did not complete under the deterministic loss profile");
```

- [x] **步骤 2：运行测试验证红灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test end_to_end
/Users/blackhook/.cargo/bin/cargo test --release -p qrbeam-core --test end_to_end large_file_survives_deterministic_damaged_channel -- --exact
```

预期：两个命令都 FAIL，原因是文件级发送和接收会话不存在。

- [x] **步骤 3：实现文件级会话和区块图状态**

公开 API 固定为：

```rust
pub enum BlockState { Missing, Partial { unique: u32, required: u32 }, Complete }
pub enum ReceiveUpdate { IgnoredDuplicate, Accepted, BlockComplete(u32), Complete(Vec<u8>) }
pub struct SendSession {
    manifest: Manifest,
    segment_encoders: Vec<SegmentEncoder>,
    timeline: Timeline,
}
pub struct ReceiveSession {
    manifest: Manifest,
    segment_decoders: Vec<SegmentDecoder>,
    completed_segments: Vec<Option<Vec<u8>>>,
    seen_frames: std::collections::HashSet<(u64, u8)>,
}
```

`SendSession::new` 验证 100 MB 上限、计算文件 BLAKE3、按 512 KiB 分块和 CRC32C、创建 Manifest 和 SegmentEncoder。`frame_for_plan` 根据 `FramePlan` 重新生成确定 payload 并编码成 Frame。`ReceiveSession::ingest` 先解析和验证 Frame，再验证 session/file/segment/符号范围，最后把符号送入对应 SegmentDecoder。完成区块写入内存中的独立 `Vec<u8>`；全部区块完成后按顺序拼接并验证 BLAKE3。第一阶段允许最终拼接保存在内存，后续存储计划把相同接口替换为分段存储后端。

- [x] **步骤 4：运行端到端测试和全量检查验证绿灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test end_to_end
/Users/blackhook/.cargo/bin/cargo test --release -p qrbeam-core --test end_to_end large_file_survives_deterministic_damaged_channel -- --exact
/Users/blackhook/.cargo/bin/cargo test --workspace
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
```

预期：端到端测试在确定性 50% 丢帧、重复、乱序和损坏条件下完成；全部测试和 Clippy 通过。

- [x] **步骤 5：Commit**

```bash
git add crates/qrbeam-core/src crates/qrbeam-core/tests/end_to_end.rs
git commit -m "feat: 打通文件光学传输协议闭环"
```

### 任务 7：冻结黄金向量和线协议文档

**文件：**
- 创建：`docs/protocol/wire-format-v1.md`
- 修改：`crates/qrbeam-core/tests/frame_wire.rs`
- 修改：`crates/qrbeam-core/tests/manifest_wire.rs`

- [x] **步骤 1：记录独立黄金向量**

把固定 Frame 和 Manifest 的完整十六进制编码写成测试常量，测试必须把新编码结果与常量直接比较，而不是只做本库 encode/decode 往返。记录向量输入字段、输出长度、CRC32C 和 BLAKE3。

- [x] **步骤 2：故意改变一个黄金向量字节并验证红灯**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core --test frame_wire golden
```

预期：FAIL，报告首个不一致字节。随后恢复正确黄金向量。

- [x] **步骤 3：编写完整线协议文档**

文档逐字节说明 56 字节帧头、所有枚举值、清单字段顺序、整数端序、CRC 覆盖规则、BLAKE3 用途、清单分片映射、RaptorQ ESI 和区块索引关系，并嵌入测试中的黄金向量。文档明确未知版本和未知枚举必须拒绝，保留字段必须为 0。

- [x] **步骤 4：运行发布前验证**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo fmt --all --check
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
/Users/blackhook/.cargo/bin/cargo test --workspace
/Users/blackhook/.cargo/bin/cargo doc --workspace --no-deps
git diff --check
```

预期：所有命令退出码为 0，测试 0 failed，文档构建无 warning。

- [x] **步骤 5：Commit**

```bash
git add docs/protocol crates/qrbeam-core/tests
git commit -m "docs: 冻结 QRBeam v1 协议黄金向量"
```

## 第一阶段验收证据

完成计划时必须记录：

- Rust、Cargo、RaptorQ 的实际版本；
- 测试总数、通过数和失败数；
- 50% 丢帧端到端测试使用的固定种子与发送帧数；
- 1 B、1 KB、1 MB 的逐字节恢复结果；
- `cargo clippy`、`cargo fmt --check`、`cargo doc` 的退出状态；
- 当前分支和最后提交 ID。

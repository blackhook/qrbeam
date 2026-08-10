# QRBeam GitHub Pages 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 `superpowers:executing-plans` 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 交付可离线安装的 GitHub Pages 发送端和手机接收端，在不改变 QRBeam 分段 RaptorQ 语义的前提下支持周期清单、100 MB 持久化接收、单码高速档和实验双码 Turbo。

**架构：** `qrbeam-core` 保持纯 Rust 协议核心，并拆出可持久化接收引擎；`qrbeam-web` 用 `wasm-bindgen` 将纯协议操作暴露给 TypeScript。`apps/qrbeam-web` 是 Vite 多页 PWA，发送端负责文件暂存、性能探测和 Canvas，接收端负责相机、ZXing Worker、OPFS/IndexedDB 和区块恢复。GitHub Pages 只交付和缓存应用资源，传输仍只通过屏幕和相机。

**技术栈：** Rust 1.97、`wasm-bindgen`、`wasm-pack`、Vite、TypeScript、Vitest、Playwright、`vite-plugin-pwa`、`qrcode`、`zxing-wasm`、OPFS、IndexedDB。

---

## 计划前提

- 先阅读 [已批准规格](../specs/2026-08-07-qrbeam-github-pages-design.md) 和 [线协议](../../protocol/wire-format-v1.md)。
- 所有 Rust 命令使用 `/Users/blackhook/.cargo/bin/cargo`，不能误用 Homebrew 的 Cargo 1.75。
- 所有 Rust 代码遵循现有 `unsafe_code = "forbid"` 和 Clippy `pedantic` 规则。
- 新增 Web 依赖必须锁定到 `apps/qrbeam-web/package-lock.json`；WASM 工具版本固定在 CI。
- 每个任务完成后只提交该任务列出的文件。任务间不得顺带格式化、重命名或重构无关文件。

## 文件结构

| 路径 | 职责 |
|---|---|
| `crates/qrbeam-core/src/constants.rs` | 协议尺寸、文字长度和每帧符号上限 |
| `crates/qrbeam-core/src/manifest.rs` | 默认档位、清单尺寸验证和分片 |
| `crates/qrbeam-core/src/receiver.rs` | 同会话周期清单的无损刷新行为 |
| `crates/qrbeam-core/src/persistent_receiver.rs` | 可确认区块持久化、恢复与 LRU 部分解码器 |
| `crates/qrbeam-core/src/manifest_carousel.rs` | 初始与周期清单轮播，不修改数据时间轴 |
| `crates/qrbeam-core/tests/*.rs` | 协议、清单、接收恢复和轮播回归测试 |
| `crates/qrbeam-cli/src/player.rs` | CLI 复用周期清单与不回退的 `Home` 行为 |
| `crates/qrbeam-cli/tests/player.rs` | CLI 轮播与快捷键测试 |
| `crates/qrbeam-web/` | 仅含 WASM 接口，不含 DOM、相机或存储 |
| `apps/qrbeam-web/` | Vite 多页 Web 应用、PWA、Worker、浏览器测试 |
| `.github/workflows/web-pages.yml` | Web 测试、构建和 GitHub Pages 部署 |
| `.github/workflows/ios-alpha.yml` | 更新后的 Rust/移动端协议兼容验证 |
| `scripts/build-web-standalone.mjs` | 生成内联资源的 `qrbeam-send.html` |
| `README.md` | 用户入口、离线方式和已验证/没验证性能说明 |

### 任务 1：提高协议高速档并限制清单尺寸

**文件：**

- 修改：`crates/qrbeam-core/src/constants.rs`
- 修改：`crates/qrbeam-core/src/manifest.rs`
- 修改：`crates/qrbeam-core/src/timeline.rs`
- 修改：`crates/qrbeam-core/tests/manifest_wire.rs`
- 修改：`crates/qrbeam-core/tests/timeline.rs`
- 修改：`docs/protocol/wire-format-v1.md`

- [ ] **步骤 1：为 11 符号档和 255 B 文字边界编写失败测试。**

```rust
#[test]
fn high_profile_accepts_eleven_symbols_per_frame() {
    let manifest = sample_manifest();
    let profile = manifest.profiles[3];
    assert_eq!(profile.symbols_per_frame, 11);
    manifest.encode().unwrap();
}

#[test]
fn manifest_rejects_a_filename_longer_than_255_utf8_bytes() {
    let mut manifest = sample_manifest();
    manifest.filename = "a".repeat(256);
    assert!(manifest.encode().is_err());
}
```

- [ ] **步骤 2：运行测试并确认现有实现失败。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core high_profile_accepts_eleven_symbols_per_frame
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core manifest_rejects_a_filename_longer_than_255_utf8_bytes
```

预期：第一个测试显示 `10 != 11`；第二个测试意外通过。

- [ ] **步骤 3：实现协议常量和默认档位。**

在 `constants.rs` 定义单一来源：

```rust
pub const MAX_SYMBOLS_PER_FRAME: u16 = 11;
pub const MAX_MANIFEST_TEXT_BYTES: usize = 255;
```

在 `manifest.rs` 删除局部 `MAX_TEXT_BYTES`，把默认档位 `id = 3` 改为：

```rust
Profile {
    id: 3,
    symbols_per_frame: 11,
    ecc: EccLevel::L,
    target_fps: 60,
    min_module_pixels: 4,
    max_qr_version: 40,
}
```

`Manifest::validate` 和 `validate_profile` 必须使用新常量；`timeline.rs` 也必须使用同一个 `MAX_SYMBOLS_PER_FRAME`，不能保留 `10` 这个魔法数字。文件名和 MIME 类型以 UTF-8 字节数限制到 `255`，Web 层负责在码点边界清理后再构造 Manifest。

- [ ] **步骤 4：更新协议文档和运行绿测。**

更新 `wire-format-v1.md` 的档位表、文字上限和最大高速帧：`56 + 11 × 256 = 2,872 B`，小于 `2,953 B`。

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core manifest_wire timeline
/Users/blackhook/.cargo/bin/cargo clippy -p qrbeam-core --all-targets -- -D warnings
```

预期：所有测试通过，Clippy 无告警。

- [ ] **步骤 5：提交协议高速档。**

```bash
git add crates/qrbeam-core/src/constants.rs crates/qrbeam-core/src/manifest.rs crates/qrbeam-core/src/timeline.rs crates/qrbeam-core/tests/manifest_wire.rs crates/qrbeam-core/tests/timeline.rs docs/protocol/wire-format-v1.md
git commit -m "feat(protocol): 支持每帧 11 个符号"
```

### 任务 2：实现周期清单轮播和无损清单刷新

**文件：**

- 创建：`crates/qrbeam-core/src/manifest_carousel.rs`
- 修改：`crates/qrbeam-core/src/lib.rs`
- 修改：`crates/qrbeam-core/src/receiver.rs`
- 创建：`crates/qrbeam-core/tests/manifest_carousel.rs`
- 修改：`crates/qrbeam-core/tests/receiver_controller.rs`
- 修改：`crates/qrbeam-cli/src/player.rs`
- 修改：`crates/qrbeam-cli/tests/player.rs`

- [ ] **步骤 1：为轮播和重复清单写失败测试。**

```rust
#[test]
fn carousel_repeats_the_complete_manifest_without_advancing_data_cursor() {
    let sender = SendSession::new("sample.bin", "", &[7; 1024], [1; 16], 1).unwrap();
    let mut carousel = ManifestCarousel::new(manifest_frames(&sender)).unwrap();
    let first_round = carousel.take_round();
    let second_round = carousel.take_round();
    assert_eq!(first_round, second_round);
}

#[test]
fn receiving_the_same_manifest_keeps_completed_blocks() {
    let mut sender = SendSession::new("sample.bin", "", &[7; 1024], [1; 16], 1).unwrap();
    let manifest = manifest_frames(&sender);
    let mut controller = ReceiverController::new();
    for frame in &manifest { controller.ingest(frame).unwrap(); }
    for _ in 0..8 {
        let frame = sender.next_frames(&[ChannelRequest { channel_id: 0, profile_id: 2, symbols_per_frame: 8 }]).unwrap().remove(0);
        let _ = controller.ingest(&frame).unwrap();
    }
    for frame in &manifest { controller.ingest(frame).unwrap(); }
    assert!(matches!(controller.snapshot().blocks[0], BlockState::Complete));
}
```

在 CLI 测试中加入：初始 `3` 秒结束后，每 `2 × fps` 个未暂停 tick 插入一整轮清单；`home()` 只请求清单轮次，不能把数据帧游标设为 `0`。

- [ ] **步骤 2：运行失败测试。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core manifest_carousel receiver_controller
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-cli player
```

预期：`ManifestCarousel` 尚不存在；当前重复清单会重新创建 `ReceiveSession`；CLI 的 `home()` 会回退时间轴。

- [ ] **步骤 3：实现轮播与接收刷新语义。**

新增纯数据结构：

```rust
pub struct ManifestCarousel {
    frames: Vec<Vec<u8>>,
    next: usize,
}

impl ManifestCarousel {
    pub fn next_frame(&mut self) -> Vec<u8> { /* 轮转克隆一帧 */ }
    pub fn round_len(&self) -> usize { self.frames.len() }
}
```

`ReceiverController::ingest_manifest` 必须先比较已存在会话的 `session_id`、`file_id` 和完整 Manifest BLAKE3。相同清单返回新增的 `ControllerUpdate::ManifestRefreshed`，只更新 `last_frame_index`，绝不替换 `session` 或 `snapshot.blocks`。不同清单保留现有行为，由 UI 决定是否请求切换。

在 `Player` 中增加 `manifest_round_remaining`、`ticks_since_manifest_round` 和 `manifest_interval_ticks = fps * 2`。`next_frame()` 在 warmup 后先服务待发清单轮次，再服务数据。暂停时不增加计数。`home()` 仅设置 `manifest_round_remaining = manifest_frames.len()`。

- [ ] **步骤 4：运行完整相关绿测。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo fmt --all -- --check
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-cli
```

预期：新增轮播、重复清单和 CLI 行为测试通过；现有端到端测试不变。

- [ ] **步骤 5：提交轮播。**

```bash
git add crates/qrbeam-core/src/lib.rs crates/qrbeam-core/src/manifest_carousel.rs crates/qrbeam-core/src/receiver.rs crates/qrbeam-core/tests/manifest_carousel.rs crates/qrbeam-core/tests/receiver_controller.rs crates/qrbeam-cli/src/player.rs crates/qrbeam-cli/tests/player.rs
git commit -m "feat(protocol): 周期重发清单并保留接收进度"
```

### 任务 3：把接收器拆成可持久化区块引擎

**文件：**

- 创建：`crates/qrbeam-core/src/persistent_receiver.rs`
- 修改：`crates/qrbeam-core/src/lib.rs`
- 修改：`crates/qrbeam-core/src/receiver.rs`
- 修改：`crates/qrbeam-core/src/session.rs`
- 修改：`crates/qrbeam-core/src/segment.rs`
- 创建：`crates/qrbeam-core/tests/persistent_receiver.rs`
- 修改：`crates/qrbeam-core/tests/end_to_end.rs`
- 修改：`crates/qrbeam-bridge/src/api/receiver.rs`
- 修改：`crates/qrbeam-bridge/tests/mobile_receiver.rs`

- [ ] **步骤 1：为区块确认、恢复重放和内存接收适配器写失败测试。**

```rust
#[test]
fn recovered_segment_is_not_complete_until_storage_acknowledges_it() {
    let mut receiver = PersistentReceiver::from_manifest(sample_manifest()).unwrap();
    let update = ingest_until_segment_ready(&mut receiver, 0);
    assert!(matches!(update, PersistentUpdate::SegmentReady { index: 0, .. }));
    assert!(matches!(receiver.block_states()[0], BlockState::Partial { .. }));
    receiver.acknowledge_segment(0).unwrap();
    assert_eq!(receiver.block_states()[0], BlockState::Complete);
}

#[test]
fn replaying_saved_frames_restores_a_partial_segment() {
    let frames = frames_for_partial_segment();
    let mut restored = PersistentReceiver::from_manifest(sample_manifest()).unwrap();
    for frame in frames { restored.ingest(&frame).unwrap(); }
    assert!(matches!(restored.block_states()[0], BlockState::Partial { .. }));
}
```

- [ ] **步骤 2：运行测试确认引擎尚不存在。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core persistent_receiver
```

预期：编译失败，提示 `PersistentReceiver` 和 `PersistentUpdate` 未定义。

- [ ] **步骤 3：实现同步、存储无关的引擎。**

创建以下稳定 API：

```rust
pub enum PersistentUpdate {
    Accepted,
    IgnoredDuplicate,
    SegmentReady { index: u32, bytes: Vec<u8> },
    ManifestRefreshed,
}

pub struct PersistentReceiver { /* manifest、状态、最多 3 个活跃 SegmentDecoder */ }

impl PersistentReceiver {
    pub fn from_manifest(manifest: Manifest) -> Result<Self, ProtocolError>;
    pub fn ingest(&mut self, frame: &[u8]) -> Result<PersistentUpdate, ProtocolError>;
    pub fn acknowledge_segment(&mut self, index: u32) -> Result<(), ProtocolError>;
    pub fn restore_completed_segment(&mut self, index: u32) -> Result<(), ProtocolError>;
    pub fn restore_partial_frames(&mut self, index: u32, frames: &[Vec<u8>]) -> Result<(), ProtocolError>;
    pub fn snapshot(&self) -> &ReceiverSnapshot;
}
```

当某个段恢复时，引擎返回字节但不把块标成 Complete。调用方写入持久化存储后调用 `acknowledge_segment`。超过 `3` 个部分段时，逐出最久未用解码器；调用方已保存的原始有效帧用于下一次访问时重建。已完成段只保留布尔状态和 CRC，不保留完整字节。

保留现有 `ReceiverController` 作为内存适配器：它用 `Vec<Option<Vec<u8>>>` 接收 `SegmentReady`，立即 `acknowledge_segment`，最后按序计算 BLAKE3 并返回既有的 `Complete(Vec<u8>)`。Flutter 生成接口继续暴露 `completedFile()`，避免本任务破坏当前移动端 UI。

- [ ] **步骤 4：运行引擎、桥接和全量核心测试。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-core
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-bridge
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
```

预期：持久化引擎能在确认前保持 Partial；桥接测试仍能获得完整文件；Clippy 无告警。

- [ ] **步骤 5：提交持久化接收边界。**

```bash
git add crates/qrbeam-core/src/lib.rs crates/qrbeam-core/src/persistent_receiver.rs crates/qrbeam-core/src/receiver.rs crates/qrbeam-core/src/session.rs crates/qrbeam-core/src/segment.rs crates/qrbeam-core/tests/persistent_receiver.rs crates/qrbeam-core/tests/end_to_end.rs crates/qrbeam-bridge/src/api/receiver.rs crates/qrbeam-bridge/tests/mobile_receiver.rs
git commit -m "refactor(core): 拆分可持久化区块接收器"
```

### 任务 4：新增 WASM 协议适配层

**文件：**

- 修改：`Cargo.toml`
- 创建：`crates/qrbeam-web/Cargo.toml`
- 创建：`crates/qrbeam-web/src/lib.rs`
- 创建：`crates/qrbeam-web/tests/web_api.rs`
- 创建：`crates/qrbeam-web/tests/web_api_browser.rs`
- 创建：`scripts/build-qrbeam-wasm.sh`

- [ ] **步骤 1：为 WASM API 写 Rust 单元测试和浏览器测试。**

```rust
#[test]
fn sender_exposes_manifest_frames_and_eleven_symbol_data_frames() {
    let mut sender = WebSender::new(sample_sender_input()).unwrap();
    assert!(!sender.manifest_frames().unwrap().is_empty());
    let frame = sender.encode_next(&segment_zero(), 3, 0).unwrap();
    assert_eq!(Frame::decode(&frame).unwrap().header.symbol_count, 11);
}

#[wasm_bindgen_test]
fn receiver_returns_a_serializable_segment_ready_event() {
    let event = WebReceiver::new(sample_manifest_bytes()).unwrap();
    assert_eq!(event.snapshot().phase(), "receiving");
}
```

- [ ] **步骤 2：运行测试确认 crate 尚不存在。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-web
```

预期：Cargo 提示 package `qrbeam-web` 不存在。

- [ ] **步骤 3：建立最小 WASM 接口。**

把 `qrbeam-web` 加入 workspace，并固定 `wasm-bindgen`、`serde` 和 `serde-wasm-bindgen` 版本。crate 类型为 `cdylib` 与 `rlib`。接口只接受和返回 `Uint8Array`、数字与序列化 DTO；不得访问 `window`、DOM、OPFS 或 IndexedDB。

核心接口如下：

```rust
#[wasm_bindgen]
pub struct WebSender { /* Web streaming planner */ }

#[wasm_bindgen]
impl WebSender {
    #[wasm_bindgen(constructor)]
    pub fn new(input: JsValue) -> Result<WebSender, JsValue>;
    pub fn manifest_frames(&self) -> Result<JsValue, JsValue>;
    pub fn next_plan(&mut self, profile_id: u8, channel_id: u8) -> Result<JsValue, JsValue>;
    pub fn encode_plan(&mut self, plan: JsValue, segment: &[u8]) -> Result<Vec<u8>, JsValue>;
}

#[wasm_bindgen]
pub struct WebReceiver { /* PersistentReceiver */ }

#[wasm_bindgen]
impl WebReceiver {
    pub fn ingest(&mut self, frame: &[u8]) -> Result<JsValue, JsValue>;
    pub fn acknowledge_segment(&mut self, index: u32) -> Result<(), JsValue>;
    pub fn restore_completed_segment(&mut self, index: u32) -> Result<(), JsValue>;
    pub fn restore_partial_frames(&mut self, index: u32, frames: JsValue) -> Result<(), JsValue>;
    pub fn snapshot(&self) -> JsValue;
}
```

`WebSender` 只持有时间轴和最多 `2` 个当前 RaptorQ 编码器；TypeScript 根据 `plan.segment_index` 从 OPFS 读出对应 `512 KiB` 段再交给 `encode_plan`。这避免把整个 100 MB 文件复制进 WASM。

- [ ] **步骤 4：构建 WASM 并运行双环境测试。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-web
wasm-pack test crates/qrbeam-web --node
wasm-pack test crates/qrbeam-web --headless --chrome
scripts/build-qrbeam-wasm.sh
```

预期：Rust 测试和两个 WASM 测试目标通过，构建产物写入 `apps/qrbeam-web/src/wasm/pkg/`。

- [ ] **步骤 5：提交 WASM 适配层。**

```bash
git add Cargo.toml Cargo.lock crates/qrbeam-web scripts/build-qrbeam-wasm.sh apps/qrbeam-web/src/wasm/pkg
git commit -m "feat(web): 添加 QRBeam WASM 协议适配层"
```

### 任务 5：建立 Vite 多页应用与离线存储抽象

**文件：**

- 创建：`apps/qrbeam-web/package.json`
- 创建：`apps/qrbeam-web/tsconfig.json`
- 创建：`apps/qrbeam-web/vite.config.ts`
- 创建：`apps/qrbeam-web/index.html`
- 创建：`apps/qrbeam-web/send/index.html`
- 创建：`apps/qrbeam-web/receive/index.html`
- 创建：`apps/qrbeam-web/src/shared/storage.ts`
- 创建：`apps/qrbeam-web/src/shared/session-record.ts`
- 创建：`apps/qrbeam-web/src/shared/storage.test.ts`
- 创建：`apps/qrbeam-web/src/shared/style.css`

- [ ] **步骤 1：为存储优先级、空间预留和会话恢复写失败测试。**

```ts
it("uses OPFS when available and IndexedDB otherwise", async () => {
  const store = await createTransferStore({ opfs: fakeOpfs(), indexedDb: fakeIndexedDb() });
  expect(store.kind).toBe("opfs");
});

it("rejects a receiver session when free space is below 120 percent", async () => {
  await expect(assertReceiveCapacity(100_000_000, 119_999_999)).rejects.toThrow("存储空间不足");
});
```

- [ ] **步骤 2：安装依赖并确认测试失败。**

运行：

```bash
cd apps/qrbeam-web
npm install
npm run test -- storage.test.ts
```

预期：测试因 `createTransferStore` 和 `assertReceiveCapacity` 未定义失败。

- [ ] **步骤 3：实现多页 Vite 基础与 Storage Coordinator。**

`vite.config.ts` 使用固定 `base: "/qrbeam/"`，并配置三个 HTML 输入。`storage.ts` 定义：

```ts
export interface TransferStore {
  readonly kind: "opfs" | "indexeddb";
  writeSourceSegment(index: number, bytes: Uint8Array): Promise<void>;
  readSourceSegment(index: number): Promise<Uint8Array>;
  appendPartialFrame(segment: number, frame: Uint8Array): Promise<void>;
  readPartialFrames(segment: number): Promise<Uint8Array[]>;
  writeCompletedSegment(index: number, bytes: Uint8Array): Promise<void>;
  listCompletedSegments(): Promise<number[]>;
  readCompletedSegment(index: number): Promise<Uint8Array>;
  saveSession(record: SessionRecord): Promise<void>;
  loadSession(sessionId: string): Promise<SessionRecord | undefined>;
  clearSession(sessionId: string): Promise<void>;
}
```

`session-record.ts` 定义可 JSON 序列化的 `SessionRecord`：清单原始字节、会话 ID、完成区块索引、部分区块索引、最后数据帧和创建时间。`assertReceiveCapacity` 使用 `navigator.storage.estimate()`，要求 `quota - usage >= Math.ceil(containerLength * 1.2)`。

页面先只显示静态标题和「离线缓存准备中」，不接入文件或相机。

- [ ] **步骤 4：验证构建、单元测试和三入口。**

运行：

```bash
cd apps/qrbeam-web
npm run test
npm run typecheck
npm run build
```

预期：Vitest、TypeScript 和生产构建通过；`dist/index.html`、`dist/send/index.html` 与 `dist/receive/index.html` 存在。

- [ ] **步骤 5：提交 Web 基础。**

```bash
git add apps/qrbeam-web/package.json apps/qrbeam-web/package-lock.json apps/qrbeam-web/tsconfig.json apps/qrbeam-web/vite.config.ts apps/qrbeam-web/index.html apps/qrbeam-web/send/index.html apps/qrbeam-web/receive/index.html apps/qrbeam-web/src/shared
git commit -m "feat(web): 建立多页应用和离线存储抽象"
```

### 任务 6：实现简洁的电脑发送端

**文件：**

- 创建：`apps/qrbeam-web/src/send/main.ts`
- 创建：`apps/qrbeam-web/src/send/sender-controller.ts`
- 创建：`apps/qrbeam-web/src/send/performance-probe.ts`
- 创建：`apps/qrbeam-web/src/send/manifest-scheduler.ts`
- 创建：`apps/qrbeam-web/src/send/qr-worker.ts`
- 创建：`apps/qrbeam-web/src/send/sender-controller.test.ts`
- 创建：`apps/qrbeam-web/src/send/manifest-scheduler.test.ts`
- 创建：`apps/qrbeam-web/src/send/performance-probe.test.ts`
- 修改：`apps/qrbeam-web/send/index.html`
- 修改：`apps/qrbeam-web/src/shared/style.css`

- [ ] **步骤 1：为周期调度、理论速率与快捷键写失败测试。**

```ts
it("inserts one complete manifest round every 2 seconds without rewinding data", () => {
  const scheduler = new ManifestScheduler({ fps: 60, manifestFrames: fiveFrames });
  expect(scheduler.next(0).kind).toBe("manifest");
  scheduler.finishWarmup();
  expect(scheduler.next(2_000).kind).toBe("manifest");
  expect(scheduler.dataCursor).toBe(0);
});

it("reports 161.92 kB/s for eleven symbols and five manifest frames", () => {
  expect(netTheoreticalBytesPerSecond({ fps: 60, symbols: 11, manifestFrames: 5 })).toBe(161_920);
});
```

- [ ] **步骤 2：运行测试确认发送模块不存在。**

运行：

```bash
cd apps/qrbeam-web
npm run test -- sender-controller.test.ts manifest-scheduler.test.ts performance-probe.test.ts
```

预期：模块解析失败。

- [ ] **步骤 3：实现文件暂存、探测、二维码渲染和最小 UI。**

`SenderController` 必须按如下顺序工作：

1. 拒绝超过 `100_000_000` 字节的 File；以流式读取方式写入 `TransferStore`，生成 Manifest 所需散列与 CRC。
2. 从 `WebSender.manifest_frames()` 获得稳定清单帧；先轮播至少 `3` 秒。
3. 用真实数据帧执行 `8 → 30 → 60 FPS` 的 `2` 秒探测；`60 FPS` 只在刷新率至少 `120 Hz` 且本地丢时隙率不超过 `5%` 时启用。
4. 每 `2,000 ms` 排入一整轮清单。调度延迟时只发一轮。
5. 将二进制帧交给 `qr-worker.ts`。Worker 使用固定 mask `4` 生成模块矩阵；主线程用关闭平滑的 Canvas 按整数倍绘制。

页面只有文件选择器、二维码、状态行和折叠 `<details>`。控制器必须注册 `Space`、`↑`、`↓`、`J`、`L`、`R`、`Home`、`Q` 与 `Esc`，其语义严格遵循规格。`Esc` 不得在普通发送状态停止传输。

理论速率函数：

```ts
export function netTheoreticalBytesPerSecond(input: {
  fps: number;
  symbols: number;
  manifestFrames: number;
  intervalSeconds?: number;
}): number {
  const interval = input.intervalSeconds ?? 2;
  return ((input.fps * interval - input.manifestFrames) * input.symbols * 256) / interval;
}
```

全屏使用根元素类名 `qr-full`，不能依赖 Element Fullscreen API。二维码画布必须满足静区和最小模块像素；空间不足时请求低一档 profile。

- [ ] **步骤 4：运行发送端自动化验证。**

运行：

```bash
cd apps/qrbeam-web
npm run test -- sender-controller.test.ts manifest-scheduler.test.ts performance-probe.test.ts
npm run typecheck
npm run build
```

预期：`60 FPS / 11 符号 / 5 清单帧` 的结果为 `161920`；所有快捷键测试和构建通过。

- [ ] **步骤 5：提交发送端。**

```bash
git add apps/qrbeam-web/src/send apps/qrbeam-web/send/index.html apps/qrbeam-web/src/shared/style.css
git commit -m "feat(web): 添加离线二维码发送端"
```

### 任务 7：实现手机接收端、Worker 池和进度恢复

**文件：**

- 创建：`apps/qrbeam-web/src/receive/main.ts`
- 创建：`apps/qrbeam-web/src/receive/receiver-controller.ts`
- 创建：`apps/qrbeam-web/src/receive/camera.ts`
- 创建：`apps/qrbeam-web/src/receive/decoder-pool.ts`
- 创建：`apps/qrbeam-web/src/receive/zxing-worker.ts`
- 创建：`apps/qrbeam-web/src/receive/throughput.ts`
- 创建：`apps/qrbeam-web/src/receive/receiver-controller.test.ts`
- 创建：`apps/qrbeam-web/src/receive/throughput.test.ts`
- 修改：`apps/qrbeam-web/receive/index.html`
- 修改：`apps/qrbeam-web/src/shared/style.css`

- [ ] **步骤 1：为 Worker 忙丢帧、存储确认和刷新恢复写失败测试。**

```ts
it("drops a camera frame while every decoder worker is busy", () => {
  const pool = new DecoderPool(2, fakeWorkerFactory());
  pool.decode(frameA); pool.decode(frameB); pool.decode(frameC);
  expect(pool.droppedFrames).toBe(1);
});

it("persists a recovered segment before acknowledging WASM", async () => {
  const receiver = await makeReceiver({ store: fakeStore(), wasm: fakeWasmSegmentReady(7) });
  await receiver.ingest(validFrame);
  expect(receiver.wasm.acknowledged).toEqual([7]);
  expect(receiver.store.completedSegments).toContain(7);
});
```

- [ ] **步骤 2：运行失败测试。**

运行：

```bash
cd apps/qrbeam-web
npm run test -- receiver-controller.test.ts throughput.test.ts
```

预期：接收模块尚不存在。

- [ ] **步骤 3：实现相机、ZXing 和持久化接收流程。**

`camera.ts` 使用 `requestVideoFrameCallback`；先尝试 iOS 所需的精确帧率，再回退理想帧率。能力存在时请求连续自动对焦，不开启闪光灯。每次重启相机递增 generation，旧 generation 的回调必须被忽略。

`DecoderPool` 创建固定数量的 `zxing-worker.ts`，所有 Worker 忙时丢弃当前相机帧。Worker 只返回通过 QR 解码的 `Uint8Array`；协议 CRC 仍由 WASM 验证。

`ReceiverController` 处理结果时严格按以下顺序执行：

```ts
const event = wasm.ingest(frame);
if (event.kind === "segment-ready") {
  await store.writeCompletedSegment(event.index, event.bytes);
  await wasm.acknowledge_segment(event.index);
}
if (event.kind === "accepted") {
  await store.appendPartialFrame(event.segmentIndex, frame);
}
```

刷新后，先载入 `SessionRecord`，调用 `restore_completed_segment`，再按区块回放 `readPartialFrames`。完成后流式读取所有完成段，通过 WASM 增量 BLAKE3 验证；只有验证通过才显示保存或分享。

页面显示相机预览、文件名、总进度、有效速度和区块图。无有效帧超过 `3` 秒时显示距离、亮度和降速建议；不清空进度。显示缺失区块号并说明在发送端按 `R` 回补。

- [ ] **步骤 4：运行接收端测试与浏览器构建。**

运行：

```bash
cd apps/qrbeam-web
npm run test -- receiver-controller.test.ts throughput.test.ts
npm run typecheck
npm run build
```

预期：忙 Worker 的帧被统计为丢弃；段先持久化后确认；刷新恢复测试通过。

- [ ] **步骤 5：提交接收端。**

```bash
git add apps/qrbeam-web/src/receive apps/qrbeam-web/receive/index.html apps/qrbeam-web/src/shared/style.css
git commit -m "feat(web): 添加持久化手机接收端"
```

### 任务 8：增加 PWA、单文件发送页和 GitHub Pages 工作流

**文件：**

- 修改：`apps/qrbeam-web/vite.config.ts`
- 创建：`apps/qrbeam-web/src/shared/pwa.ts`
- 创建：`apps/qrbeam-web/src/shared/pwa.test.ts`
- 创建：`scripts/build-web-standalone.mjs`
- 创建：`scripts/tests/build-web-standalone-test.mjs`
- 创建：`.github/workflows/web-pages.yml`
- 修改：`.gitignore`

- [ ] **步骤 1：为预缓存清单和单文件发送页写失败测试。**

```ts
it("pre-caches every application wasm and worker asset", () => {
  expect(requiredPrecacheAssets(manifest)).toEqual([]);
});
```

```js
assert.match(output, /<script[^>]*>.*WebSender/s);
assert.doesNotMatch(output, /src="[^"#]+\.js"/);
assert.doesNotMatch(output, /https?:\/\//);
```

- [ ] **步骤 2：运行测试确认功能尚不存在。**

运行：

```bash
cd apps/qrbeam-web
npm run test -- pwa.test.ts
node scripts/tests/build-web-standalone-test.mjs
```

预期：缺少 PWA 辅助模块和单文件产物。

- [ ] **步骤 3：实现 PWA 和部署。**

配置 `vite-plugin-pwa`，将 `index.html`、`send/index.html`、`receive/index.html`、QRBeam WASM、ZXing WASM、所有 Worker、图标和 CSS 纳入 Workbox precache。缓存完成后才显示「离线可用」。禁止把 OPFS 或 IndexedDB 用户文件写入 Cache Storage。

`build-web-standalone.mjs` 从生产发送页构建产物读取脚本、样式、WASM 和 Worker，生成 `dist-standalone/qrbeam-send.html`。该页面不能引用网络 URL，也不能生成手机接收页。

工作流分为 `test` 和 `deploy`：

```yaml
on:
  push:
    branches: [main]
    paths: ["apps/qrbeam-web/**", "crates/qrbeam-web/**", "crates/qrbeam-core/**", ".github/workflows/web-pages.yml"]
jobs:
  test:
    # 安装 Rust 1.97、Node 锁定版本、wasm-pack；运行 Rust/WASM/Web 测试和 build
  deploy:
    needs: test
    permissions: { pages: write, id-token: write }
    # upload-pages-artifact 与 deploy-pages
```

使用 GitHub Pages 官方 Actions，并把 Vite `base` 保持为 `/qrbeam/`。在仓库 Settings 启用 GitHub Actions 作为 Pages Source 后，部署 URL 必须写入 README。

- [ ] **步骤 4：验证断网产物和工作流语法。**

运行：

```bash
cd apps/qrbeam-web
npm run test -- pwa.test.ts
npm run build
node ../../scripts/build-web-standalone.mjs
node ../../scripts/tests/build-web-standalone-test.mjs
```

再用 Playwright 打开生产构建，拦截除初始页面外的全部网络请求；重新加载发送页和接收页，预期应用资源来自 Service Worker 缓存。

- [ ] **步骤 5：提交离线与部署能力。**

```bash
git add apps/qrbeam-web/vite.config.ts apps/qrbeam-web/src/shared/pwa.ts apps/qrbeam-web/src/shared/pwa.test.ts scripts/build-web-standalone.mjs scripts/tests/build-web-standalone-test.mjs .github/workflows/web-pages.yml .gitignore
git commit -m "feat(web): 添加离线 PWA 与 GitHub Pages 部署"
```

### 任务 9：执行跨端兼容回归与更新用户文档

**文件：**

- 修改：`apps/qrbeam_mobile/test/receive_view_model_test.dart`
- 修改：`crates/qrbeam-bridge/tests/mobile_receiver.rs`
- 修改：`README.md`
- 修改：`docs/research/2026-08-04-decimen-comparison.md`
- 创建：`docs/testing/2026-08-10-web-device-matrix.md`
- 修改：`.github/workflows/ios-alpha.yml`

- [ ] **步骤 1：为移动端桥接的重复清单回归添加失败测试。**

```rust
#[test]
fn same_session_manifest_refresh_keeps_mobile_completed_blocks() {
    let mut receiver = MobileReceiver::new();
    ingest_first_block_to_completion(&mut receiver);
    receiver.ingest(repeated_manifest_frame()).unwrap();
    assert_eq!(receiver.snapshot().blocks[0].kind, MobileBlockKind::Complete);
}
```

- [ ] **步骤 2：运行移动端和桥接测试确认失败原因。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo test -p qrbeam-bridge mobile_receiver
cd apps/qrbeam_mobile
flutter test test/receive_view_model_test.dart
```

预期：在任务 2 的同会话刷新语义未被桥接接收器保留时失败；任务 2 完成后该测试必须通过。

- [ ] **步骤 3：更新移动 UI 映射、CI 和文档。**

移动端必须把相同会话清单刷新视为非错误，不清空区块图。`MobileReceiver::ingest` 保持现有 `MobileSnapshot` 返回形状，Dart 页无需了解新增的 `ControllerUpdate::ManifestRefreshed`。CI 必须同时运行 Rust workspace 测试、Flutter 测试、Flutter analyze、Web 测试和 Web build。

README 新增：

- GitHub Pages 的首页、发送页和接收页 URL；
- 首次联网缓存后可离线使用；
- 单文件 `qrbeam-send.html` 的用途；
- `60 Hz` 与 `120 Hz` 的理论档位；
- `168.96 kB/s` 是理论数据载荷，实际手机速度「没验证」直到设备矩阵完成；
- 旧 Alpha 1 与新 11 符号档不兼容；
- 无网络不等于加密。

设备矩阵文档预先列出 iOS Safari/PWA、Android Chrome/PWA、Chrome、Edge、Safari、Firefox、60/120 Hz、文件尺寸和观察字段；未执行项必须标记「没验证」。

- [ ] **步骤 4：运行所有可自动化的跨端检查。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo fmt --all -- --check
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
/Users/blackhook/.cargo/bin/cargo test --workspace
cd apps/qrbeam_mobile && flutter test && flutter analyze
cd ../qrbeam-web && npm run test && npm run typecheck && npm run build
```

预期：全部命令退出码为 `0`。真实相机和屏幕测量不在此步骤宣称通过。

- [ ] **步骤 5：提交兼容与文档。**

```bash
git add apps/qrbeam_mobile/test/receive_view_model_test.dart crates/qrbeam-bridge/tests/mobile_receiver.rs README.md docs/research/2026-08-04-decimen-comparison.md docs/testing/2026-08-10-web-device-matrix.md .github/workflows/ios-alpha.yml
git commit -m "docs: 记录 Web 跨端兼容与验收矩阵"
```

### 任务 10：发布前浏览器端到端与真机验收

**文件：**

- 创建：`apps/qrbeam-web/playwright.config.ts`
- 创建：`apps/qrbeam-web/e2e/offline.spec.ts`
- 创建：`apps/qrbeam-web/e2e/send-receive.spec.ts`
- 修改：`docs/testing/2026-08-10-web-device-matrix.md`
- 修改：`README.md`

- [ ] **步骤 1：先写失败的浏览器端到端用例。**

```ts
test('a late receiver obtains the manifest within one two-second interval', async ({ page }) => {
  await page.goto('/qrbeam/send/');
  await startDeterministicStream(page);
  await page.waitForTimeout(1_250);
  const receiver = await newReceiverPage(page.context());
  await feedFramesFromSender(receiver, { durationMs: 2_000 });
  await expect(receiver.getByText('报告_fj2_hash.pdf')).toBeVisible();
});

test('offline reload serves cached sender and receiver pages', async ({ context, page }) => {
  await primePwaCache(page);
  await context.setOffline(true);
  await page.goto('/qrbeam/send/');
  await expect(page.getByText('发送文件')).toBeVisible();
});
```

- [ ] **步骤 2：运行端到端用例确认失败。**

运行：

```bash
cd apps/qrbeam-web
npx playwright test e2e/offline.spec.ts e2e/send-receive.spec.ts
```

预期：用例因为测试夹具与页面控制器尚未连通而失败。

- [ ] **步骤 3：实现确定性测试夹具和真实解码回归。**

测试夹具从 `SenderController` 捕获原始帧，使用同一 QR Worker 生成图像；将图像以缩放、旋转、亮度与轻度模糊变体交给真实 ZXing Worker。不要模拟协议接受结果。为相机 API 提供只在测试构建启用的 `FrameSource`，生产构建只使用 `MediaStreamTrack`。

在设备矩阵中记录真实项目：设备型号、系统、浏览器、发送显示器刷新率、相机分辨率、文件大小、有效 FPS、丢帧率、有效速度、最终 BLAKE3 与测试日期。`120 Hz + 10 MB` 的 `100 kB/s` 是验收目标；没有记录时必须写「没验证」。

- [ ] **步骤 4：运行发布前全套验证。**

运行：

```bash
/Users/blackhook/.cargo/bin/cargo fmt --all -- --check
/Users/blackhook/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
/Users/blackhook/.cargo/bin/cargo test --workspace
cd apps/qrbeam-web && npm run test && npm run typecheck && npm run build && npx playwright test
node ../../scripts/build-web-standalone.mjs
node ../../scripts/tests/build-web-standalone-test.mjs
```

预期：所有自动化命令退出码为 `0`；真机表格只陈述已经记录的结果。

- [ ] **步骤 5：提交端到端验收，并创建预发布。**

```bash
git add apps/qrbeam-web/playwright.config.ts apps/qrbeam-web/e2e docs/testing/2026-08-10-web-device-matrix.md README.md
git commit -m "test(web): 添加离线传输端到端验收"
git push origin feat/ios-receiver
```

在 GitHub Actions 的 Pages 部署成功后，创建新的预发布标签，上传 `qrbeam-send.html`、CLI、IPA 和 APK。发布说明只写已记录的真实速度；理论速率必须标明「理论数据载荷」。

## 规格覆盖自检

| 规格需求 | 实现任务 |
|---|---|
| 同一 Pages 站点、PWA 和单文件发送页 | 任务 5、8 |
| Rust WASM 共用协议核心 | 任务 3、4 |
| 100 MB 与持久化恢复 | 任务 3、5、7 |
| 11 符号高速档与刷新率限制 | 任务 1、6 |
| 每 2 秒完整清单与晚加入 | 任务 2、6、10 |
| 简洁发送页、快捷键、回补、全屏 | 任务 6 |
| 手机 Worker、区块图、速度和完成保存 | 任务 7 |
| 错误、后台、存储与校验门控 | 任务 3、5、7 |
| 双二维码实验档 | 任务 6、10 |
| CI、部署、真实设备矩阵与发布说明 | 任务 8、9、10 |

实施前再次确认本文没有未落实标记；新增 API 名称必须与后续任务一致。

# QRBeam iOS Alpha 1 实现计划

> **面向 AI 代理的工作者：** 按本计划在当前会话内逐任务执行。步骤使用复选框（`- [ ]`）跟踪进度，并遵循测试先行、最小实现、频繁提交。

**目标：** 交付一个可由电脑终端连续显示 QRBeam 二维码、由 iPhone 扫描恢复文件并展示区块状态的 Alpha 1，同时产出可复现的 iOS 构建和 IPA 归档。

**架构：** Rust `qrbeam-core` 继续持有协议、清单、RaptorQ 和完整性校验；新增统一接收控制器及 Rust/Flutter 桥。Rust CLI 使用固定 mask 4 的二进制二维码在终端备用屏幕播放。Flutter 只负责 ZXing-C++ 相机帧、接收状态 UI 和文件分享。

**技术栈：** Rust 1.97.1、qrcodegen 1.8、crossterm、Flutter 3.44、flutter_zxing 2.3、flutter_rust_bridge 2.x、Xcode 26.2、iOS 13+

---

## 文件结构

- `crates/qrbeam-core/src/receiver.rs`：清单和数据帧的统一接收状态机。
- `crates/qrbeam-core/src/lib.rs`：导出统一接收 API。
- `crates/qrbeam-core/tests/receiver_controller.rs`：统一接收器的端到端行为测试。
- `crates/qrbeam-cli/src/main.rs`：命令行参数、终端生命周期和键盘事件。
- `crates/qrbeam-cli/src/player.rs`：清单阶段、数据阶段和时间轴播放。
- `crates/qrbeam-cli/src/render.rs`：固定 mask 4 的二维码矩阵和 ANSI 半块渲染。
- `crates/qrbeam-cli/tests/qr_render.rs`：矩阵确定性、容量和静区测试。
- `crates/qrbeam-bridge/src/api/receiver.rs`：面向 Flutter 的不透明接收对象和 DTO。
- `apps/qrbeam_mobile/lib/features/receive/`：相机、状态模型、区块图和接收页面。
- `apps/qrbeam_mobile/test/`：状态模型与 Widget 测试。
- `scripts/build-ios-alpha.sh`：无交互、可复现的 iOS release/IPA 构建入口。
- `.github/workflows/ios-alpha.yml`：无签名 iOS 构建验证与产物上传。

### 任务 1：统一 Rust 接收控制器

- [x] **步骤 1：编写失败测试**

在 `crates/qrbeam-core/tests/receiver_controller.rs` 构造 `SendSession`，先重复喂入全部清单帧，再乱序、丢帧地喂入数据和修复帧，断言状态依次为 `WaitingManifest`、`Receiving`、`Complete`，完成字节与原文件完全一致。

- [x] **步骤 2：验证红灯**

运行：`cargo test -p qrbeam-core --test receiver_controller`

预期：因 `ReceiverController` 尚不存在而编译失败。

- [x] **步骤 3：最小实现**

创建以下公开模型，并让 `ingest` 先执行 `Frame::decode`，清单阶段交给 `ManifestAssembler`，数据阶段交给 `ReceiveSession`：

```rust
pub enum ReceiverPhase { WaitingManifest, Receiving, Complete }

pub struct ReceiverSnapshot {
    pub phase: ReceiverPhase,
    pub filename: Option<String>,
    pub total_bytes: Option<u64>,
    pub blocks: Vec<BlockState>,
    pub last_frame_index: Option<u64>,
}

pub struct ReceiverController {
    assembler: ManifestAssembler,
    session: Option<ReceiveSession>,
    snapshot: ReceiverSnapshot,
}
```

- [x] **步骤 4：验证绿灯和回归**

运行：`cargo test -p qrbeam-core --test receiver_controller && cargo test --workspace`

预期：新增测试通过；既有 37 个非忽略测试继续通过。

- [x] **步骤 5：提交**

提交信息：`feat(core): 添加统一接收控制器`

### 任务 2：固定二维码矩阵与终端渲染

- [x] **步骤 1：编写失败测试**

在 `crates/qrbeam-cli/tests/qr_render.rs` 验证：相同二进制帧产生完全相同矩阵；矩阵 mask 为 4；四边各有 4 模块静区；过大载荷返回结构化错误；ANSI 输出每两个模块行只占一个终端行。

- [x] **步骤 2：验证红灯**

运行：`cargo test -p qrbeam-cli --test qr_render`

预期：因 CLI crate 和 `QrMatrix` 尚不存在而失败。

- [x] **步骤 3：最小实现**

使用 `qrcodegen::QrSegment::make_bytes` 和高级编码入口锁定 mask 4、关闭纠错自动升级：

```rust
let segments = [QrSegment::make_bytes(frame)];
let qr = QrCode::encode_segments_advanced(
    &segments,
    QrCodeEcc::Medium,
    Version::MIN,
    Version::MAX,
    Some(Mask::new(4)),
    false,
)?;
```

渲染器用 ANSI 前景/背景色和 `▀` 合并上下两个模块行，并把 4 模块静区计入矩阵。

- [x] **步骤 4：验证绿灯**

运行：`cargo test -p qrbeam-cli --test qr_render`

预期：全部通过。

- [x] **步骤 5：提交**

提交信息：`feat(cli): 添加固定二维码终端渲染`

### 任务 3：可操作的稳定档发送器

- [x] **步骤 1：编写失败测试**

为 `Player` 使用可控时钟编写测试：清单帧循环至少覆盖 3 秒；随后稳定档每帧 1 个符号；暂停不推进时间轴；后退 100 帧重放历史；退出恢复终端状态。

- [x] **步骤 2：验证红灯**

运行：`cargo test -p qrbeam-cli player`

预期：因 `Player` 尚不存在而失败。

- [x] **步骤 3：最小实现**

实现 `qrbeam send <file> [--fps 1..=8]`。读取不超过 100 MB 的文件，生成随机会话 ID；先循环显示清单，再持续显示数据/修复帧。支持 `Space` 暂停、`J/L` 前后 100 帧、`Home` 回到首帧、`Q/Esc` 退出。

- [x] **步骤 4：验证绿灯**

运行：`cargo test -p qrbeam-cli && cargo run -p qrbeam-cli -- --help`

预期：测试通过，帮助文本列出 `send` 和快捷键。

- [x] **步骤 5：提交**

提交信息：`feat(cli): 添加稳定档二维码文件发送器`

### 任务 4：Flutter/Rust 类型安全桥

- [x] **步骤 1：编写失败测试**

在 `qrbeam-bridge` 中从黄金清单和数据帧调用公开桥 API，断言 DTO 中的阶段、文件名、总大小、区块状态和完成字节与核心一致。

- [x] **步骤 2：验证红灯**

运行：`cargo test -p qrbeam-bridge`

预期：桥 crate 尚不存在而失败。

- [x] **步骤 3：生成脚手架并做最小实现**

确认根目录 `rust-toolchain.toml` 固定为本机已验证的 `1.97.1`，并加入 iOS device/simulator targets。再用 Flutter 官方模板创建 `apps/qrbeam_mobile`，用稳定版 flutter_rust_bridge 2.11.1 的默认 Cargokit 后端接入工作区中的 `qrbeam-bridge`。不采用仍处于 beta 的 Native Assets 后端。桥 API 只暴露：

```rust
pub struct MobileReceiver { inner: ReceiverController }
impl MobileReceiver {
    pub fn new() -> Self;
    pub fn ingest(&mut self, frame: Vec<u8>) -> Result<MobileSnapshot, String>;
    pub fn completed_file(&self) -> Option<Vec<u8>>;
}
```

- [x] **步骤 4：验证绿灯**

运行：`cargo test -p qrbeam-bridge && flutter analyze`

预期：桥测试通过，Dart 静态检查无错误。

- [x] **步骤 5：提交**

提交信息：`feat(mobile): 接入 Rust 接收核心`

### 任务 5：iOS 相机接收界面与区块图

- [x] **步骤 1：编写失败的 Dart 测试**

状态模型测试覆盖：灰/黄/绿/红四种区块；部分区块计算总体进度且完成前不超过 99%；10 秒滑动窗口带宽；只有 Rust 返回 `Complete` 时显示“接收完成”。Widget 测试覆盖等待清单、接收中和完成三种页面。

- [x] **步骤 2：验证红灯**

运行：`flutter test`

预期：因接收状态模型和页面尚不存在而失败。

- [x] **步骤 3：最小实现**

使用 `flutter_zxing` 2.3 的 `ReaderWidget`，只接受 QR Code，并把解码结果的原始 `bytes` 送入 Rust。相机回调忙时直接丢帧，不排队。页面包含取景框、文件名和大小、总进度、区块图、瞬时/稳定带宽、最近帧号；完成后把字节写入临时文件并调用系统分享面板。

- [x] **步骤 4：iOS 配置**

设置 bundle id `com.blackhook.qrbeam`、最低 iOS 13、`NSCameraUsageDescription=QRBeam 需要相机读取电脑屏幕上的二维码`，不申请麦克风、通讯录、蓝牙和网络权限。

- [x] **步骤 5：验证绿灯**

运行：`flutter test && flutter analyze && flutter build ios --simulator`

预期：测试和静态检查通过，模拟器 `Runner.app` 构建成功。

- [x] **步骤 6：提交**

提交信息：`feat(ios): 添加二维码文件接收界面`

### 任务 6：构建、体积和离线检查

- [x] **步骤 1：编写构建脚本验收测试**

脚本必须在缺少签名时明确输出 `UNSIGNED` 并产出模拟器 App 或无签名 archive；存在签名时才调用 `flutter build ipa`。脚本失败时保持非零退出码，不吞掉 Xcode 错误。

- [x] **步骤 2：实现构建入口与 GitHub Actions**

`scripts/build-ios-alpha.sh` 依次运行 Rust 测试、Flutter 测试、analyze、iOS release 构建和体积检查。`.github/workflows/ios-alpha.yml` 在 macOS runner 上执行相同验证并上传无签名构建产物。

- [x] **步骤 3：完整验证**

运行：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
flutter test
flutter analyze
flutter build ios --release --no-codesign
```

预期：全部成功；release App 小于 100 MB；二进制中不包含 HTTP/HTTPS 服务地址。

- [x] **步骤 4：生成 IPA 状态**

若钥匙串和描述文件可用，运行 `flutter build ipa --release` 并记录 SHA-256；否则从无签名 release `Runner.app` 生成明确命名的 `QRBeam-Alpha1-unsigned.ipa`，同时标记它必须经用户自己的 Apple 证书签名后才能安装。

- [x] **步骤 5：提交并推送**

提交信息：`ci(ios): 添加 Alpha 1 构建与产物校验`

推送 `feat/ios-receiver`，在 GitHub 创建 Draft PR，附测试结果、IPA 签名状态、文件大小和 SHA-256。

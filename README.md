# QRBeam

QRBeam 通过屏幕上连续变化的二维码，把电脑文件离线传到手机。传输过程不需要局域网、蓝牙、数据线、麦克风或电脑摄像头。

当前 Alpha 1 包含：

- Rust 协议核心：文件清单、分段、RaptorQ 丢帧恢复、CRC32C 区块校验和 BLAKE3 文件校验。
- 电脑 CLI 发送端：终端连续显示二维码，支持暂停、回退 100 帧和回到文件清单。
- iOS 接收端：相机扫码、灰/黄/绿/红区块图、总体进度、实时带宽和完成后保存/分享。
- Flutter/Rust 共用接收核心，Android 工程和相机权限已接入；Android 安装包尚未做真机验证。

## 电脑发送

需要 Rust 1.97 或更高版本。进入项目目录后运行：

```bash
cargo run --release -p qrbeam-cli -- send /path/to/file --fps 4
```

发送时快捷键：

- `Space`：暂停或继续
- `J`：回退 100 帧，便于补帧
- `L`：前进 100 帧
- `Home`：重新发送 3 秒文件清单
- `Q` 或 `Esc`：停止

建议先用 `--fps 2` 测试，再逐步提高到 4–8 FPS。手机显示“接收完成”后，在电脑按 `Q` 停止。

## iOS 构建

```bash
scripts/build-ios-alpha.sh
```

脚本会运行全部 Rust/Flutter 测试、静态检查、Release 构建、100 MB 体积限制和二进制网络地址检查。

- 本机有 Apple 签名证书和描述文件时，生成 `dist/QRBeam-Alpha1-signed.ipa`。
- 缺少签名时，生成 `dist/QRBeam-Alpha1-unsigned.ipa`。无签名 IPA 不能直接安装，必须先用你自己的 Apple 证书重签名。

相机真实扫码、不同手机屏幕与电脑终端组合的稳定带宽目前没验证；这是 Alpha 1 最重要的实测项。

## 开发验证

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd apps/qrbeam_mobile
flutter test
flutter analyze
flutter build ios --simulator
```

协议说明见 [`docs/protocol/wire-format-v1.md`](docs/protocol/wire-format-v1.md)，与 Decimen 的差异见 [`docs/research/2026-08-04-decimen-comparison.md`](docs/research/2026-08-04-decimen-comparison.md)。

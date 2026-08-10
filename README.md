# QRBeam

QRBeam 通过屏幕上连续变化的二维码，把电脑文件离线传到手机。传输过程不需要局域网、蓝牙、数据线、麦克风或电脑摄像头。

当前 Alpha 1 包含：

- Rust 协议核心：文件清单、分段、RaptorQ 丢帧恢复、CRC32C 区块校验和 BLAKE3 文件校验。
- 电脑 CLI 发送端：终端连续显示二维码，支持暂停、回退 100 帧和回到文件清单。
- iOS 接收端：相机扫码、灰/黄/绿/红区块图、总体进度、实时带宽和完成后保存/分享。
- Flutter/Rust 共用接收核心，Android 工程和相机权限已接入；Android 安装包尚未做真机验证。
- GitHub Pages 网页端：电脑发送和手机接收都复用 Rust/WASM 协议核心；首次联网打开后可作为 PWA 离线使用。

## 网页发送与接收

网页应用位于 `apps/qrbeam-web`，包含 `/`、`/send/` 和 `/receive/` 三个入口。发送端会先探测屏幕刷新率，在 `8 / 30 / 60 FPS` 中选择上限：只有约 120 Hz 的屏幕才启用 60 FPS。稳定清单使用 M 纠错，数据使用 L 纠错；首次发送至少 3 秒清单，之后每 2 秒插入一轮清单。

手机接收端使用相机识别二进制 QRBeam 帧，先把恢复出的区块写入 IndexedDB；只有全部区块的 CRC32C、总长度和完整文件 BLAKE3 都验证通过，才会显示“接收完成”并允许保存。页面刷新会恢复清单、完成区块和部分区块帧。

本地运行：

```bash
cd apps/qrbeam-web
npm ci
npm run dev
```

生产构建：

```bash
cd apps/qrbeam-web
npm run build
```

需要不依赖本地服务器的电脑发送页时，可运行 `npm run build:standalone`，生成 `apps/qrbeam-web/qrbeam-send.html`；该文件可直接用浏览器打开。

每帧 11 个 `256 B` 符号在 60 FPS 下的协议理论上限是 `168.96 kB/s`；扣除每 2 秒一轮清单后的数值取决于清单片数。例如 5 片清单时是 `161.92 kB/s`。手机相机、屏幕、距离和环境光下的实际吞吐尚未完成真机验证，不能把理论值当成实测值。

网页端已发布到 GitHub Pages：[电脑发送页](https://blackhook.github.io/qrbeam/send/) 和 [手机接收页](https://blackhook.github.io/qrbeam/receive/)。首次联网打开并安装 PWA 后，可在没有网络的环境中继续使用。

## 电脑发送

需要 Rust 1.97 或更高版本。进入项目目录后运行：

```bash
rustup run 1.97.1 cargo run --release -p qrbeam-cli -- send /path/to/file --fps 4
```

发送时快捷键：

- `Space`：暂停或继续
- `J`：回退 100 帧，便于补帧
- `L`：前进 100 帧
- `Home`：立即重发一整轮文件清单，不回退数据时间轴
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

## Android 构建

安装 Android SDK 并设置 `ANDROID_HOME` 或 `ANDROID_SDK_ROOT` 后运行：

```bash
scripts/build-android-alpha.sh
```

脚本会生成 `dist/QRBeam-Alpha1-android.apk`，运行测试、静态检查和 100 MB 体积限制。APK 面向现代 `arm64-v8a` Android 手机与 x86_64 模拟器；当前 RaptorQ 上游在 32 位 ARM 的 NEON 实现无法通过稳定 Rust 编译，因此暂不包含 `armeabi-v7a`。当前 Android release 使用项目的 debug 签名，仅适合测试安装；发布前需要替换成正式签名配置。

## 开发验证

```bash
rustup run 1.97.1 cargo fmt --all -- --check
rustup run 1.97.1 cargo clippy --workspace --all-targets -- -D warnings
rustup run 1.97.1 cargo test --workspace
cd apps/qrbeam_mobile
flutter test
flutter analyze
flutter build ios --simulator
```

协议说明见 [`docs/protocol/wire-format-v1.md`](docs/protocol/wire-format-v1.md)，与 Decimen 的差异见 [`docs/research/2026-08-04-decimen-comparison.md`](docs/research/2026-08-04-decimen-comparison.md)。

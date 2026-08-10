# 自适应网页二维码播放器实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 让 `/send/` 根据当前可见二维码区域自动选择手机可扫的 QRBeam 档位，并提供扫码优先的全屏界面。

**架构：** Rust/WASM 发送器按 manifest profile 生成帧；网页端用同一 QR 编码器测得真实模块数，并以画布物理像素和 6 px/模块阈值选最高合格 profile。播放器负责测量、重测和重绘，不重建 session 或时间轴。

**技术栈：** Rust 1.97、wasm-bindgen、TypeScript、Vite、`qrcode`、Playwright。

---

## 文件结构

- 修改：`crates/qrbeam-web/src/lib.rs` — 提供按 profile ID 获取下一帧的 WASM API。
- 修改：`crates/qrbeam-web/src/lib.rs` 的测试或新增 `crates/qrbeam-web/tests/profile_frames.rs` — 锁定 profile 帧头与时间轴行为。
- 创建：`apps/qrbeam-web/src/send/adaptive_profile.ts` — 纯函数，依据真实 QR 模块数和画布物理尺寸选档位。
- 创建：`apps/qrbeam-web/tests/adaptive_profile.spec.ts` — 浏览器端档位选择回归测试。
- 修改：`apps/qrbeam-web/src/send/main.ts` — 全屏播放器、测量、二维码实际编码、重测与状态栏。
- 修改：`apps/qrbeam-web/src/shared/style.css` — 扫码优先的响应式播放器样式。
- 修改：`apps/qrbeam-web/tests/send.spec.ts` — 窄/宽视口、状态栏、全屏按钮与实际档位断言。

### 任务 1：WASM 按 profile 生成协议帧

**文件：**
- 修改：`crates/qrbeam-web/src/lib.rs`
- 测试：`crates/qrbeam-web/tests/profile_frames.rs`

- [ ] **步骤 1：编写失败的 Rust 测试**

```rust
#[test]
fn requested_profile_is_encoded_into_the_frame_header() {
    let mut sender = sender_with_small_file();
    let bytes = sender.preview_profile_frame(0).unwrap();
    let frame = qrbeam_core::frame::Frame::decode(&bytes).unwrap();
    assert_eq!(frame.header.profile_id, 0);
    assert_eq!(frame.header.symbol_count, 1);
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`rustup run 1.97.1 cargo test -p qrbeam-web --test profile_frames requested_profile_is_encoded_into_the_frame_header -- --exact`

预期：FAIL，提示 `preview_profile_frame` 未定义。

- [ ] **步骤 3：实现按 profile 选择 channel**

```rust
pub fn next_profile_frame(&mut self, profile_id: u8) -> Result<Vec<u8>, JsValue> {
    let profile = self.inner.manifest().profiles.iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| JsValue::from_str("unknown profile"))?;
    let channel = ChannelRequest {
        channel_id: 0,
        profile_id,
        symbols_per_frame: u16::from(profile.symbols_per_frame),
    };
    self.inner.next_frames(&[channel]).map_err(js_error)
        .and_then(|mut frames| frames.pop().ok_or_else(|| JsValue::from_str("missing frame")))
}

pub fn preview_profile_frame(&self, profile_id: u8) -> Result<Vec<u8>, JsValue> {
    let mut preview = self.inner.clone();
    next_frame_for_profile(&mut preview, profile_id)
}
```

- [ ] **步骤 4：运行 Rust 测试验证通过**

运行：`rustup run 1.97.1 cargo test -p qrbeam-web --test profile_frames`

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add crates/qrbeam-web/src/lib.rs crates/qrbeam-web/tests/profile_frames.rs
git commit -m "feat(wasm): 支持按协议档位生成发送帧"
```

### 任务 2：实现可测试的自适应档位选择

**文件：**
- 创建：`apps/qrbeam-web/src/send/adaptive_profile.ts`
- 创建：`apps/qrbeam-web/tests/adaptive_profile.spec.ts`

- [ ] **步骤 1：编写失败的浏览器测试**

```ts
expect(choosePlaybackProfile({
  profiles: [high, medium, safe],
  canvasCssPixels: 480,
  devicePixelRatio: 2,
  refreshRate: 60,
  moduleCounts: new Map([[3, 177], [1, 121], [0, 77]]),
})).toMatchObject({ profileId: 0, modulePhysicalPixels: 11.4, fps: 8 });
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd apps/qrbeam-web && node ./node_modules/@playwright/test/cli.js test tests/adaptive_profile.spec.ts`

预期：FAIL，提示模块未导出 `choosePlaybackProfile`。

- [ ] **步骤 3：实现纯选择函数**

```ts
export function choosePlaybackProfile(input: AdaptiveInput): AdaptiveSelection | AdaptiveFailure {
  for (const profile of [...input.profiles].sort((a, b) => b.symbolsPerFrame - a.symbolsPerFrame)) {
    const modules = input.moduleCounts.get(profile.id);
    const modulePhysicalPixels = input.canvasCssPixels * input.devicePixelRatio / (modules + 8);
    if (modules && modulePhysicalPixels >= 6) {
      return { profileId: profile.id, fps: Math.min(profile.targetFps, input.refreshRate), modulePhysicalPixels, modules };
    }
  }
  return { reason: "二维码模块不足 6 个物理像素" };
}
```

- [ ] **步骤 4：运行选择测试验证通过**

运行：`cd apps/qrbeam-web && node ./node_modules/@playwright/test/cli.js test tests/adaptive_profile.spec.ts`

预期：PASS，覆盖高档位、降档和尺寸不足三种情况。

- [ ] **步骤 5：Commit**

```bash
git add apps/qrbeam-web/src/send/adaptive_profile.ts apps/qrbeam-web/tests/adaptive_profile.spec.ts
git commit -m "feat(web): 按屏幕模块尺寸选择发送档位"
```

### 任务 3：接入发送页与扫码优先界面

**文件：**
- 修改：`apps/qrbeam-web/src/send/main.ts`
- 修改：`apps/qrbeam-web/src/shared/style.css`
- 修改：`apps/qrbeam-web/tests/send.spec.ts`

- [ ] **步骤 1：编写失败的 Playwright 测试**

```ts
test("窄窗口自动降档并展示物理模块尺寸", async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 720 });
  await page.goto("send/");
  await page.locator("#file").setInputFiles(fixture);
  await expect(page.locator("#profile-status")).toContainText("模块");
  await expect(page.locator("#qr-player")).toHaveClass(/active/);
});
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd apps/qrbeam-web && node ./node_modules/@playwright/test/cli.js test tests/send.spec.ts`

预期：FAIL，缺少 `#profile-status` 和 `#qr-player`。

- [ ] **步骤 3：实现播放器测量与重测**

```ts
const measure = () => Math.floor(Math.min(window.innerWidth - 32, window.innerHeight - 132));
const reconfigure = async () => {
  const cssPixels = measure();
  const samples = await Promise.all(profiles.map(async profile => [profile.id, qrModules(sender.preview_profile_frame(profile.id))]));
  selection = choosePlaybackProfile({ profiles, canvasCssPixels: cssPixels, devicePixelRatio, refreshRate, moduleCounts: new Map(samples) });
  renderProfileStatus(selection);
};
window.addEventListener("resize", () => void reconfigure());
document.addEventListener("fullscreenchange", () => void reconfigure());
```

播放器必须使用只读预览测量；正式定时器首次取帧即为时间轴第一帧。二维码画布 CSS 边长等于测得边长，位图边长为 CSS 边长乘 DPR，且不再写死 `1080`。

- [ ] **步骤 4：实现全屏布局**

```css
.send-player.active { min-height: 100dvh; display: grid; grid-template-rows: 1fr auto; }
.send-player .qr-wrap { min-height: 0; padding: 12px; }
.send-player canvas { width: min(100%, calc(100dvh - 132px)); height: auto; }
.player-bar { min-height: 84px; display: flex; align-items: center; justify-content: space-between; }
```

保留 `Space`、`J/L`、`Home`、`R`、`Q` 快捷键；增加进入/退出全屏按钮；所有状态都在底栏可见。

- [ ] **步骤 5：运行网页验证**

运行：`cd apps/qrbeam-web && npm run typecheck && npm test && npm run build && npm run build:standalone`

预期：PASS，生产 PWA 与独立发送页均能构建。

- [ ] **步骤 6：Commit**

```bash
git add apps/qrbeam-web/src/send/main.ts apps/qrbeam-web/src/shared/style.css apps/qrbeam-web/tests/send.spec.ts
git commit -m "feat(web): 使用自适应全屏二维码播放器"
```

### 任务 4：全仓库验证与发布

**文件：**
- 修改：`README.md`
- 更新：`gh-pages` 静态产物

- [ ] **步骤 1：补充用户说明**

在 README 网页发送章节写明：档位按实时模块物理像素自动选择，6 px/模块为最低阈值；全屏可获得更高吞吐；状态栏展示最终选档。

- [ ] **步骤 2：运行跨端验证**

运行：

```bash
rustup run 1.97.1 cargo fmt --all -- --check
rustup run 1.97.1 cargo clippy --workspace --all-targets -- -D warnings
rustup run 1.97.1 cargo test --workspace
cd apps/qrbeam-web && npm run typecheck && npm test && npm run build && npm run build:standalone
cd ../qrbeam_mobile && flutter test && flutter analyze
```

预期：所有命令退出码为 0。

- [ ] **步骤 3：发布静态网页**

构建 `apps/qrbeam-web/dist`，同步到 `gh-pages` 时排除 `.git`，提交 `deploy: 发布自适应二维码播放器`，推送后等待 Pages build 为 `built`，并验证 `/`、`/send/`、`/receive/` 返回 200。

- [ ] **步骤 4：Commit README 与推送功能分支**

```bash
git add README.md
git commit -m "docs: 说明网页端自适应二维码档位"
git push -u origin feat/adaptive-qr-player
```

## 自检

- 规格中的全屏布局、物理像素测量、候选档位、6 px 门槛、刷新率限制、重测、失败提示、状态栏、协议连续性及测试均分别由任务 1–4 覆盖。
- 计划未含待定项；函数名 `choosePlaybackProfile`、`next_profile_frame`、`profileId` 和 6 px 阈值跨任务一致。

# QRBeam 网页发送端自适应二维码播放器

## 目标

解决当前网页发送端固定 `1080px` 画布、固定高载荷 `11` 符号帧导致 QR 版本过高、模块过小、手机难以识别的问题。发送端必须根据**当前实际可见扫码区域**自动选择最高可扫档位，并在窗口或全屏状态变化后重新评估。

## 范围

- `/send/` 改成扫码优先的全屏播放器：二维码占可用高度主体，文件与传输状态固定在窄底栏；设置与快捷键置入可展开面板。
- 每次选择文件、`resize`、进入/退出全屏时测量：可用正方形 CSS 边长、`devicePixelRatio`、实际画布像素边长、编码后 QR 模块数。
- 候选协议档位为现有 manifest 中的 `1 / 5 / 8 / 11` 符号帧。逐个生成真实二进制 QRBeam 帧并用同一个 QR 编码器测量模块数。
- 仅选择模块边长至少 `6` 个物理像素的最高吞吐档位；每档的 FPS 不超过协议档位上限及探测到的显示刷新率。
- 若 `1` 符号档位也小于 6 个物理像素，则不开始传输，明确提示用户全屏、放大窗口或提高系统显示缩放比例。
- 清单帧使用 M 纠错；数据帧使用档位定义的纠错级别。周期清单、session、全局帧序号、Repair、定位快捷键语义保持不变。
- 底栏显示实际选中的“符号/帧、FPS、QR 版本、模块物理像素”，让用户知道当前是否处于可扫范围。

## 架构

1. Rust/WASM `WebSender` 暴露按 manifest profile ID 获取下一帧的 API；旧 `next_turbo_frame` 移除，避免浏览器临时固定帧格式与协议档位脱节。
2. TypeScript 的 `choosePlaybackProfile` 是纯函数：接受可用画布边长、DPR、刷新率和编码测量结果，返回档位/FPS或不可发送原因。
3. TypeScript 的 QR 测量使用真实待发送二进制帧调用 `QRCode.create`，以其 `modules.size` 计算 `canvasPhysicalPixels / (modules + 2 × quietZoneModules)`；不以字节数估计版本。
4. `AdaptivePlayer` 管理定时器与重测：重测前停止定时器，重绘当前可见帧或新一帧后恢复；不重建 `WebSender`，因此 session 和时间轴连续。

## 失败处理

- QR 编码或 profile 生成失败：不启动定时器，底栏显示具体错误。
- 可用尺寸不足：显示“当前窗口只能达到 X px/模块；需要至少 6 px/模块”，并提供进入全屏按钮。
- resize 期间选择到更低档位：立即降低符号数/FPS；更高档位只在下一帧生效，不回放或跳过时间轴。

## 验收与测试

- Rust 测试：不同 profile ID 生成的帧头 `profile_id` 与 `symbol_count` 匹配 manifest，且时间轴连续。
- TypeScript 单元测试：给定模块数、DPR、可用边长、刷新率，选择正确的最高合格档位；不合格时拒绝发送。
- Playwright：发送页在模拟的窄视口中显示低档位和模块尺寸；在宽视口中可以提升档位；底栏和全屏控制存在。
- 现有 Rust workspace、网页 typecheck/build/PWA/Playwright、Flutter 测试与 iOS/Android Release 构建继续通过。

## 非目标

- 不改变 QRBeam v1 线格式、文件大小上限、RaptorQ 参数或手机接收协议。
- 不把手机端摄像头识别成功率伪装成浏览器侧的保证；实机扫描仍需不同屏幕与手机组合验证。

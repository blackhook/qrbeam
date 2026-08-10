import QRCode from "qrcode";
import init, { WebSender } from "../wasm/pkg/qrbeam_web";
import { choosePlaybackProfile, type AdaptiveSelection, type PlaybackProfile } from "./adaptive_profile";
import "../shared/style.css";

const MAX_FILE_BYTES = 100_000_000;
const profiles: Array<PlaybackProfile & { ecc: "L" | "M" }> = [
  { id: 0, symbolsPerFrame: 1, targetFps: 8, ecc: "M" },
  { id: 1, symbolsPerFrame: 5, targetFps: 24, ecc: "L" },
  { id: 2, symbolsPerFrame: 8, targetFps: 30, ecc: "L" },
  { id: 3, symbolsPerFrame: 11, targetFps: 60, ecc: "L" },
];

const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `<section class="send-player" id="qr-player"><header class="player-header"><p class="eyebrow">QRBEAM / SEND</p><label class="file-label" for="file">选择文件（最大 100 MB）</label><input id="file" type="file"/><button class="secondary fullscreen" id="fullscreen" title="进入全屏">全屏</button></header><div class="qr-wrap"><canvas id="qr"></canvas></div><footer class="player-bar"><div><strong id="filename">等待文件</strong><p class="status" id="status">选择文件后自动评估当前屏幕可扫档位。</p><p class="note" id="profile-status">模块尺寸将在开始前测量</p></div><div class="player-actions"><button id="pause" class="secondary">暂停</button><button id="stop" class="secondary">停止</button></div></footer><details class="player-help"><summary>控制与补帧</summary><p><kbd>Space</kbd> 暂停　<kbd>J/L</kbd> 回退/前进 100 帧　<kbd>Home</kbd> 重发文件信息　<kbd>R</kbd> 指定区块 Repair　<kbd>Q</kbd> 停止</p><p>档位基于当前可见画布、设备像素比和真实 QR 模块数自动选择；至少 6 物理像素/模块才开始发送。</p></details></section>`;

const player = document.querySelector<HTMLElement>("#qr-player")!;
const input = document.querySelector<HTMLInputElement>("#file")!;
const canvas = document.querySelector<HTMLCanvasElement>("#qr")!;
const filename = document.querySelector<HTMLElement>("#filename")!;
const status = document.querySelector<HTMLElement>("#status")!;
const profileStatus = document.querySelector<HTMLElement>("#profile-status")!;
const pause = document.querySelector<HTMLButtonElement>("#pause")!;
const stop = document.querySelector<HTMLButtonElement>("#stop")!;
const fullscreen = document.querySelector<HTMLButtonElement>("#fullscreen")!;

let timer = 0;
let resizeTimer = 0;
let paused = false;
let sender: WebSender | undefined;
let manifests: Uint8Array[] = [];
let manifestCursor = 0;
let warmup = 0;
let sinceManifest = 0;
let selection: AdaptiveSelection | undefined;
let repairing = false;

const profileById = (id: number) => profiles.find(profile => profile.id === id)!;
const probeRefreshRate = () => new Promise<number>(resolve => {
  const started = performance.now();
  let frames = 0;
  const sample = (now: number) => {
    frames += 1;
    if (now - started < 1_000) requestAnimationFrame(sample);
    else resolve(Math.max(8, Math.round(frames / ((now - started) / 1_000))));
  };
  requestAnimationFrame(sample);
});

function canvasCssPixels() {
  return Math.floor(Math.min(window.innerWidth - 32, window.innerHeight - 146));
}

function qrModuleCount(frame: Uint8Array, ecc: "L" | "M") {
  return QRCode.create([{ data: frame, mode: "byte" }], { errorCorrectionLevel: ecc }).modules.size;
}

async function draw(frame: Uint8Array, ecc: "L" | "M") {
  const side = canvasCssPixels();
  const physicalSide = Math.floor(side * window.devicePixelRatio);
  canvas.width = physicalSide;
  canvas.height = physicalSide;
  await QRCode.toCanvas(canvas, [{ data: frame, mode: "byte" }], {
    errorCorrectionLevel: ecc,
    margin: 4,
    width: physicalSide,
  });
  canvas.style.width = `${side}px`;
  canvas.style.height = `${side}px`;
}

async function configurePlayback() {
  if (!sender) return false;
  const side = canvasCssPixels();
  if (side < 120) {
    selection = undefined;
    profileStatus.textContent = "窗口过小，请放大窗口或进入全屏后再发送";
    return false;
  }
  const refreshRate = await probeRefreshRate();
  const previews = profiles.map(profile => ({
    profile,
    frame: sender!.preview_profile_frame(profile.id),
  }));
  const choice = choosePlaybackProfile({
    canvasCssPixels: side,
    devicePixelRatio: window.devicePixelRatio,
    refreshRate,
    profiles,
    moduleCounts: previews.map(({ profile, frame }) => [profile.id, qrModuleCount(frame, profile.ecc)]),
  });
  if (choice.kind === "unavailable") {
    selection = undefined;
    profileStatus.textContent = `当前仅 ${choice.modulePhysicalPixels.toFixed(1)} px/模块；需要至少 6 px。请全屏或放大窗口。`;
    status.textContent = "尚未开始发送";
    return false;
  }
  selection = choice;
  const profile = profileById(choice.profileId);
  const preview = previews.find(item => item.profile.id === choice.profileId)!.frame;
  profileStatus.textContent = `${profile.symbolsPerFrame} 符号/帧 · ${choice.fps} FPS · QR ${choice.modules} 模块 · ${choice.modulePhysicalPixels.toFixed(1)} px/模块`;
  await draw(preview, profile.ecc);
  return true;
}

async function tick() {
  if (paused || !sender || !selection) return;
  if (warmup < selection.fps * 3 || sinceManifest >= selection.fps * 2) {
    const frame = manifests[manifestCursor++ % manifests.length];
    if (warmup < selection.fps * 3) warmup += 1;
    else sinceManifest = 0;
    status.textContent = `发送文件信息 · ${manifestCursor}/${manifests.length}`;
    await draw(frame, "M");
    return;
  }
  const profile = profileById(selection.profileId);
  const frame = sender.next_profile_frame(selection.profileId);
  sinceManifest += 1;
  status.textContent = repairing ? "Repair 补帧中" : "发送中";
  await draw(frame, profile.ecc);
}

async function begin() {
  clearInterval(timer);
  const configured = await configurePlayback();
  if (!configured || !selection) return;
  player.classList.add("active");
  timer = window.setInterval(() => void tick(), 1_000 / selection.fps);
  void tick();
}

input.onchange = async () => {
  const file = input.files?.[0];
  if (!file) return;
  if (file.size > MAX_FILE_BYTES) {
    status.textContent = "文件超过 100 MB 上限";
    return;
  }
  await init();
  filename.textContent = file.name;
  status.textContent = "正在测量当前屏幕与二维码模块…";
  const data = new Uint8Array(await file.arrayBuffer());
  sender?.free();
  sender = new WebSender({
    filename: file.name,
    mime_type: file.type,
    data,
    session_id: crypto.getRandomValues(new Uint8Array(16)),
    file_id: crypto.getRandomValues(new Uint32Array(1))[0],
  });
  manifests = (sender.manifest_frames() as number[][]).map(frame => new Uint8Array(frame));
  manifestCursor = warmup = sinceManifest = 0;
  paused = false;
  pause.textContent = "暂停";
  await begin();
};

function scheduleReconfigure() {
  if (!sender) return;
  clearTimeout(resizeTimer);
  resizeTimer = window.setTimeout(() => { void begin(); }, 180);
}

pause.onclick = () => {
  paused = !paused;
  pause.textContent = paused ? "继续" : "暂停";
};
stop.onclick = () => {
  clearInterval(timer);
  sender?.free();
  sender = undefined;
  selection = undefined;
  player.classList.remove("active");
  canvas.width = 0;
  canvas.height = 0;
  canvas.style.width = "0px";
  canvas.style.height = "0px";
  filename.textContent = "等待文件";
  status.textContent = "已停止";
  profileStatus.textContent = "模块尺寸将在开始前测量";
};
fullscreen.onclick = async () => {
  if (document.fullscreenElement) await document.exitFullscreen();
  else await player.requestFullscreen();
};
window.addEventListener("resize", scheduleReconfigure);
document.addEventListener("fullscreenchange", scheduleReconfigure);
window.addEventListener("keydown", event => {
  if (event.key === " ") { event.preventDefault(); pause.click(); }
  if (event.key === "Home") sinceManifest = selection ? selection.fps * 2 : 0;
  if (event.key.toLowerCase() === "j") sender?.seek_back(100);
  if (event.key.toLowerCase() === "l") sender?.seek_forward(100);
  if (event.key.toLowerCase() === "r" && sender) {
    if (repairing) { sender.stop_repair(); repairing = false; }
    else {
      const value = window.prompt("输入手机区块图中的缺块编号", "0");
      if (value !== null) { sender.start_repair(Number(value)); repairing = true; }
    }
  }
  if (event.key.toLowerCase() === "q") stop.click();
});

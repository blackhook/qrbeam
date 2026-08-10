import QRCode from "qrcode";
import init, { WebSender } from "../wasm/pkg/qrbeam_web";
import "../shared/style.css";

const MAX_FILE_BYTES = 100_000_000;
const FPS = 60;
const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `<section class="panel"><p class="eyebrow">QRBEAM / SEND</p><h1>发送文件</h1><label class="file-label" for="file">选择文件（最大 100 MB）</label><input id="file" type="file"/><div class="qr-wrap"><canvas id="qr" width="1080" height="1080"></canvas></div><p class="status" id="status">选择文件后先显示文件信息二维码 3 秒。</p><div class="row"><button id="pause" class="secondary">暂停</button><button id="stop" class="secondary">停止</button></div><details><summary>控制与性能</summary><p><kbd>Space</kbd> 暂停　<kbd>Home</kbd> 重发文件信息　<kbd>Q</kbd> 停止</p><p>当前：Rust/WASM RaptorQ · 每帧 11 符号 · 60 FPS（需 120 Hz 显示器才建议启用）。</p></details></section>`;
const input = document.querySelector<HTMLInputElement>("#file")!;
const canvas = document.querySelector<HTMLCanvasElement>("#qr")!;
const status = document.querySelector<HTMLParagraphElement>("#status")!;
const pause = document.querySelector<HTMLButtonElement>("#pause")!;
const stop = document.querySelector<HTMLButtonElement>("#stop")!;
let timer = 0, paused = false, sender: WebSender | undefined, manifests: Uint8Array[] = [], manifestCursor = 0, warmup = 0, sinceManifest = 0;
async function draw(frame: Uint8Array) { await QRCode.toCanvas(canvas, [{ data: frame, mode: "byte" }], { errorCorrectionLevel: "L", margin: 4, width: 1080 }); }
async function tick() { if (paused || !sender) return; let frame: Uint8Array; if (warmup < FPS * 3 || sinceManifest >= FPS * 2) { frame = manifests[manifestCursor++ % manifests.length]; if (warmup < FPS * 3) warmup++; else sinceManifest = 0; status.textContent = `发送文件信息 · ${manifestCursor}/${manifests.length}`; } else { frame = sender.next_turbo_frame(); sinceManifest++; status.textContent = `发送中 · 60 FPS · 11 符号/帧`; } await draw(frame); }
input.onchange = async () => { const file = input.files?.[0]; if (!file) return; if (file.size > MAX_FILE_BYTES) { status.textContent = "文件超过 100 MB 上限"; return; } await init(); const data = new Uint8Array(await file.arrayBuffer()); sender = new WebSender({ filename: file.name, mime_type: file.type, data: Array.from(data), session_id: Array.from(crypto.getRandomValues(new Uint8Array(16))), file_id: crypto.getRandomValues(new Uint32Array(1))[0] }); manifests = (sender.manifest_frames() as number[][]).map(frame => new Uint8Array(frame)); manifestCursor = warmup = sinceManifest = 0; clearInterval(timer); timer = window.setInterval(() => void tick(), 1000 / FPS); void tick(); };
pause.onclick = () => { paused = !paused; pause.textContent = paused ? "继续" : "暂停"; };
stop.onclick = () => { clearInterval(timer); sender?.free(); sender = undefined; status.textContent = "已停止"; };
window.addEventListener("keydown", event => { if (event.key === " ") { event.preventDefault(); pause.click(); } if (event.key === "Home") sinceManifest = FPS * 2; if (event.key.toLowerCase() === "q") stop.click(); });

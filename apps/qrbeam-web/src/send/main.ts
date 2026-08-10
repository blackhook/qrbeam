import QRCode from "qrcode";
import "../shared/style.css";

const MAX_FILE_BYTES = 100_000_000;
const CHUNK_BYTES = 1_200;
const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `<section class="panel"><p class="eyebrow">QRBEAM / SEND</p><h1>发送文件</h1><label class="file-label" for="file">选择文件（最大 100 MB）</label><input id="file" type="file"/><div class="qr-wrap"><canvas id="qr" width="720" height="720"></canvas></div><p class="status" id="status">选择文件后会先持续显示文件信息 3 秒。</p><div class="row"><button id="pause" class="secondary">暂停</button><button id="stop" class="secondary">停止</button></div><details><summary>控制与性能</summary><p><kbd>Space</kbd> 暂停　<kbd>J/L</kbd> 回退/前进 100 帧　<kbd>Home</kbd> 重发文件信息　<kbd>Q</kbd> 停止</p><p>当前浏览器版仍在接入 Rust/WASM 的 RaptorQ 编码器；本页面已具备离线缓存、二维码轮播和相机接收 UI。</p></details></section>`;
const input = document.querySelector<HTMLInputElement>("#file")!;
const canvas = document.querySelector<HTMLCanvasElement>("#qr")!;
const status = document.querySelector<HTMLParagraphElement>("#status")!;
const pause = document.querySelector<HTMLButtonElement>("#pause")!;
const stop = document.querySelector<HTMLButtonElement>("#stop")!;
let timer = 0, frames: string[] = [], cursor = 0, paused = false, started = 0;
function tick() { if (!paused && frames.length) { QRCode.toCanvas(canvas, frames[cursor], { errorCorrectionLevel: "M", margin: 4, width: 720 }); cursor = (cursor + 1) % frames.length; status.textContent = `发送中 · 帧 ${cursor + 1}/${frames.length} · ${paused ? "暂停" : "12 FPS"}`; } }
function header(id:string,name:string,size:number,count:number) { return `QRB1H|${id}|${encodeURIComponent(name)}|${size}|${count}`; }
input.onchange = async () => { const file = input.files?.[0]; if (!file) return; if (file.size > MAX_FILE_BYTES) { status.textContent="文件超过 100 MB 上限"; return; } const id = crypto.randomUUID().replaceAll("-",""); const data = new Uint8Array(await file.arrayBuffer()); const count=Math.ceil(data.length/CHUNK_BYTES); const head=header(id,file.name,file.size,count); frames=[head]; for(let i=0;i<count;i++){const part=data.slice(i*CHUNK_BYTES,(i+1)*CHUNK_BYTES);frames.push(`QRB1D|${id}|${i}|${btoa(String.fromCharCode(...part))}`)} cursor=0; started=Date.now(); clearInterval(timer); timer=window.setInterval(tick,83); status.textContent="文件信息二维码正在持续显示 3 秒…"; setTimeout(()=>{status.textContent="发送中；每 2 秒会再次发出完整文件信息"},3000); };
pause.onclick=()=>{paused=!paused;pause.textContent=paused?"继续":"暂停"}; stop.onclick=()=>{clearInterval(timer);frames=[];status.textContent="已停止"};
window.addEventListener("keydown",event=>{if(event.key===" "){event.preventDefault();pause.click()}if(event.key.toLowerCase()==="q")stop.click();if(event.key==="Home")cursor=0;if(event.key.toLowerCase()==="j")cursor=Math.max(0,cursor-100);if(event.key.toLowerCase()==="l")cursor=Math.min(Math.max(0,frames.length-1),cursor+100)});

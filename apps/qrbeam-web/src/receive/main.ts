import { BrowserMultiFormatReader } from "@zxing/browser";
import init, { WebManifestAssembler, WebReceiver } from "../wasm/pkg/qrbeam_web";
import { blockClass } from "./block_state";
import "../shared/style.css";

type ReceiverEvent = { kind: string; index?: number; bytes?: number[] };

const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `<section class="panel"><p class="eyebrow">QRBEAM / RECEIVE</p><h1>接收文件</h1><button id="camera">打开相机</button><video class="camera" id="video" playsinline muted></video><p class="status" id="status">等待文件信息二维码</p><div class="progress"><i id="bar" style="width:0%"></i></div><div class="blocks" id="blocks"></div><button class="secondary" id="save" disabled>保存文件</button><details><summary>接收说明</summary><p>绿色：已安全写入本机存储。橙色：正在解码。全部区块和完整文件哈希都验证通过后，才能保存原文件。</p></details></section>`;

const video = document.querySelector<HTMLVideoElement>("#video")!;
const status = document.querySelector<HTMLParagraphElement>("#status")!;
const blocks = document.querySelector<HTMLDivElement>("#blocks")!;
const bar = document.querySelector<HTMLElement>("#bar")!;
const save = document.querySelector<HTMLButtonElement>("#save")!;

let assembler: WebManifestAssembler;
let receiver: WebReceiver | undefined;
let manifest: Uint8Array | undefined;
let filename = "received.bin";
let originalLength = 0;
let segmentCount = 0;
let done = new Set<number>();
let partial = new Set<number>();
let verified = false;

const db = await new Promise<IDBDatabase>((resolve, reject) => {
  const request = indexedDB.open("qrbeam-receiver", 3);
  request.onupgradeneeded = () => {
    if (!request.result.objectStoreNames.contains("segments")) request.result.createObjectStore("segments");
    if (!request.result.objectStoreNames.contains("meta")) request.result.createObjectStore("meta");
    if (!request.result.objectStoreNames.contains("partial")) request.result.createObjectStore("partial");
  };
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error);
});

const writeSegment = (index: number, bytes: Uint8Array) => new Promise<void>((resolve, reject) => {
  const tx = db.transaction("segments", "readwrite");
  tx.objectStore("segments").put(bytes, index);
  tx.oncomplete = () => resolve();
  tx.onerror = () => reject(tx.error);
});
const readSegment = (index: number) => new Promise<Uint8Array>((resolve, reject) => {
  const request = db.transaction("segments").objectStore("segments").get(index);
  request.onsuccess = () => resolve(new Uint8Array(request.result));
  request.onerror = () => reject(request.error);
});
const writeMeta = (key: string, value: unknown) => new Promise<void>((resolve, reject) => {
  const tx = db.transaction("meta", "readwrite");
  tx.objectStore("meta").put(value, key);
  tx.oncomplete = () => resolve();
  tx.onerror = () => reject(tx.error);
});
const readMeta = <T>(key: string) => new Promise<T | undefined>((resolve, reject) => {
  const request = db.transaction("meta").objectStore("meta").get(key);
  request.onsuccess = () => resolve(request.result as T | undefined);
  request.onerror = () => reject(request.error);
});
const readPartial = (index: number) => new Promise<number[][]>((resolve, reject) => {
  const request = db.transaction("partial").objectStore("partial").get(index);
  request.onsuccess = () => resolve((request.result as number[][] | undefined) ?? []);
  request.onerror = () => reject(request.error);
});
const appendPartial = async (index: number, bytes: Uint8Array) => {
  const frames = await readPartial(index);
  frames.push(Array.from(bytes));
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction("partial", "readwrite");
    tx.objectStore("partial").put(frames, index);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
};
const clearPartial = (index: number) => new Promise<void>((resolve, reject) => {
  const tx = db.transaction("partial", "readwrite");
  tx.objectStore("partial").delete(index);
  tx.oncomplete = () => resolve();
  tx.onerror = () => reject(tx.error);
});

function redraw() {
  const visible = Math.min(segmentCount, 200);
  blocks.innerHTML = Array.from({ length: visible }, (_, index) => `<i class="${blockClass(index, done, partial)}"></i>`).join("");
  const percent = segmentCount ? (done.size / segmentCount) * 100 : 0;
  bar.style.width = `${percent}%`;
  status.textContent = segmentCount ? `${filename} · ${done.size}/${segmentCount} 区块 · ${Math.round(percent)}%` : "等待文件信息二维码";
  if (done.size === segmentCount && segmentCount) {
    status.textContent = verified ? `接收完成并已校验：${filename}` : `区块已齐全，正在校验：${filename}`;
    save.disabled = !verified;
  }
}

function parseManifest(bytes: Uint8Array) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  originalLength = Number(view.getBigUint64(28, true));
  const filenameLength = view.getUint16(104, true);
  segmentCount = view.getUint32(108, true);
  filename = new TextDecoder().decode(bytes.slice(112, 112 + filenameLength));
}

async function verifyCompleteFile() {
  if (!receiver || done.size !== segmentCount || !segmentCount) return;
  redraw();
  try {
    const segments = await Promise.all(Array.from({ length: segmentCount }, (_, index) => readSegment(index)));
    receiver.verify_persisted_segments(segments);
    verified = true;
    redraw();
  } catch (error) {
    verified = false;
    save.disabled = true;
    status.textContent = `文件校验失败，请用 Repair 补帧：${String(error)}`;
  }
}

async function startSession(bytes: Uint8Array) {
  manifest = bytes;
  parseManifest(bytes);
  verified = false;
  receiver = new WebReceiver(bytes);
  for (const index of done) receiver.restore_completed_segment(index);
  await writeMeta("manifest", Array.from(bytes));
  await writeMeta("done", Array.from(done));
  redraw();
}

async function ingest(bytes: Uint8Array, replaying = false) {
  if (bytes[5] === 1) {
    if (receiver) return;
    const complete = assembler.push(bytes) as number[] | undefined;
    if (complete) await startSession(new Uint8Array(complete));
    return;
  }
  if (!receiver) return;
  const segment = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(40, true);
  const event = receiver.ingest(bytes) as ReceiverEvent;
  if (event.kind === "accepted" && !done.has(segment)) {
    partial.add(segment);
    if (!replaying) await appendPartial(segment, bytes);
    redraw();
  }
  if (event.kind === "segment-ready" && event.index !== undefined && event.bytes) {
    await writeSegment(event.index, new Uint8Array(event.bytes));
    receiver.acknowledge_segment(event.index);
    done.add(event.index);
    partial.delete(event.index);
    await clearPartial(event.index);
    await writeMeta("done", Array.from(done));
    redraw();
    if (done.size === segmentCount) await verifyCompleteFile();
  }
}

await init();
assembler = new WebManifestAssembler();
const savedManifest = await readMeta<number[]>("manifest");
const savedDone = await readMeta<number[]>("done");
if (savedManifest) {
  done = new Set(savedDone ?? []);
  await startSession(new Uint8Array(savedManifest));
  for (let index = 0; index < segmentCount; index += 1) {
    if (!done.has(index)) {
      const frames = await readPartial(index);
      if (frames.length) partial.add(index);
      for (const frame of frames) await ingest(new Uint8Array(frame), true);
    }
  }
  if (done.size === segmentCount) await verifyCompleteFile();
  else status.textContent = `已恢复 ${filename}，继续扫描补齐区块`;
}

document.querySelector<HTMLButtonElement>("#camera")!.onclick = async () => {
  const reader = new BrowserMultiFormatReader();
  await reader.decodeFromVideoDevice(undefined, video, result => {
    if (!result) return;
    const raw = (result as unknown as { getRawBytes(): Uint8Array }).getRawBytes();
    void ingest(raw).catch(error => { status.textContent = `接收错误：${String(error)}`; });
  });
  status.textContent = "相机已开启，正在识别 QRBeam 帧";
};

save.onclick = async () => {
  if (!receiver || !verified) { status.textContent = "请等待完整文件校验通过"; return; }
  const segments = await Promise.all(Array.from({ length: segmentCount }, (_, index) => readSegment(index)));
  try {
    receiver.verify_persisted_segments(segments);
  } catch (error) {
    verified = false;
    redraw();
    status.textContent = `文件校验失败，请继续补帧：${String(error)}`;
    return;
  }
  const blob = new Blob(segments.map(segment => new Uint8Array(segment)), { type: "application/octet-stream" });
  if (blob.size !== originalLength) { status.textContent = "文件长度校验失败，请继续补帧"; return; }
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
};

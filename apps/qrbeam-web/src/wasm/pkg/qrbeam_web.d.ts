/* tslint:disable */
/* eslint-disable */
export class WebReceiver {
  free(): void;
  snapshot_json(): string;
  acknowledge_segment(index: number): void;
  constructor(manifest: Uint8Array);
  ingest(frame: Uint8Array): any;
}
export class WebSender {
  free(): void;
  manifest_frames(): any;
  next_turbo_frame(): Uint8Array;
  constructor(input: any);
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_webreceiver_free: (a: number, b: number) => void;
  readonly __wbg_websender_free: (a: number, b: number) => void;
  readonly webreceiver_acknowledge_segment: (a: number, b: number) => [number, number];
  readonly webreceiver_ingest: (a: number, b: number, c: number) => [number, number, number];
  readonly webreceiver_new: (a: number, b: number) => [number, number, number];
  readonly webreceiver_snapshot_json: (a: number) => [number, number];
  readonly websender_manifest_frames: (a: number) => [number, number, number];
  readonly websender_new: (a: any) => [number, number, number];
  readonly websender_next_turbo_frame: (a: number) => [number, number, number, number];
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_export_4: WebAssembly.Table;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;

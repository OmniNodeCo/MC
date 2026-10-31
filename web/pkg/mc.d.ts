/* tslint:disable */
/* eslint-disable */

export function boot(): void;

/**
 * Click in CSS pixels; used by the block palette.
 */
export function click(x: number, y: number): void;

export function fps(): number;

/**
 * Advance and draw one frame. Returns true when the world should be saved.
 */
export function frame(dt_seconds: number): boolean;

export function inventory_open(): boolean;

export function mark_saved(): void;

export function mouse_button(button: number, down: boolean): void;

export function mouse_delta(dx: number, dy: number): void;

export function persist_bytes(): Uint8Array;

/**
 * The pointer is captured, or the block palette is open (both mean "playing").
 */
export function pointer_lock(locked: boolean): void;

export function resize(width: number, height: number): void;

export function restore_bytes(data: Uint8Array | number[]): boolean;

export function set_key(code: string, down: boolean): void;

export function set_render_distance(distance: number): void;

/**
 * Create the renderer for `canvas` and start the game loop.
 */
export function start(canvas: HTMLCanvasElement, width: number, height: number, seed: number): Promise<void>;

export function wheel(delta: number): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly boot: () => void;
    readonly click: (a: number, b: number) => void;
    readonly fps: () => number;
    readonly frame: (a: number) => number;
    readonly inventory_open: () => number;
    readonly mark_saved: () => void;
    readonly mouse_button: (a: number, b: number) => void;
    readonly mouse_delta: (a: number, b: number) => void;
    readonly persist_bytes: () => [number, number];
    readonly pointer_lock: (a: number) => void;
    readonly resize: (a: number, b: number) => void;
    readonly restore_bytes: (a: number, b: number) => number;
    readonly set_key: (a: number, b: number, c: number) => void;
    readonly set_render_distance: (a: number) => void;
    readonly start: (a: any, b: number, c: number, d: number) => any;
    readonly wheel: (a: number) => void;
    readonly wgpu_render_bundle_draw: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wgpu_render_bundle_draw_indexed: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wgpu_render_bundle_set_pipeline: (a: number, b: bigint) => void;
    readonly wgpu_render_bundle_draw_indirect: (a: number, b: bigint, c: bigint) => void;
    readonly wgpu_render_bundle_set_bind_group: (a: number, b: number, c: bigint, d: number, e: number) => void;
    readonly wgpu_render_bundle_set_vertex_buffer: (a: number, b: number, c: bigint, d: bigint, e: bigint) => void;
    readonly wgpu_render_bundle_set_push_constants: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wgpu_render_bundle_draw_indexed_indirect: (a: number, b: bigint, c: bigint) => void;
    readonly wgpu_render_bundle_insert_debug_marker: (a: number, b: number) => void;
    readonly wgpu_render_bundle_pop_debug_group: (a: number) => void;
    readonly wgpu_render_bundle_set_index_buffer: (a: number, b: bigint, c: number, d: bigint, e: bigint) => void;
    readonly wgpu_render_bundle_push_debug_group: (a: number, b: number) => void;
    readonly wasm_bindgen_a5825ee7b5dff72f___convert__closures_____invoke___js_sys_638ea35702822545___Function_fn_wasm_bindgen_a5825ee7b5dff72f___JsValue_____wasm_bindgen_a5825ee7b5dff72f___sys__Undefined___js_sys_638ea35702822545___Function_fn_wasm_bindgen_a5825ee7b5dff72f___JsValue_____wasm_bindgen_a5825ee7b5dff72f___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_a5825ee7b5dff72f___convert__closures_____invoke___wasm_bindgen_a5825ee7b5dff72f___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_a5825ee7b5dff72f___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_a5825ee7b5dff72f___convert__closures_____invoke___wasm_bindgen_a5825ee7b5dff72f___JsValue______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_a5825ee7b5dff72f___convert__closures_____invoke___wasm_bindgen_a5825ee7b5dff72f___JsValue______true__19: (a: number, b: number, c: any) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
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

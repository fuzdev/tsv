/**
 * FFI bindings to native tsv library
 *
 * Uses Deno.dlopen to call the Rust library directly for maximum performance.
 */

import { ffi_library_path } from './tsv_artifacts.ts';
import { BaseImplementation, goal_for, type Language, LANGUAGES, type ParseGoal } from './types.ts';
import { assert_binding_reports_rejection } from './reject_probe.ts';

// FFI symbol definitions.
//
// The source and out-length arguments are passed as explicit `pointer`s
// rather than the `buffer` parameter type. Deno 2.8's `buffer` fast-call
// marshalling intermittently handed the native side a stale/wrong source
// pointer under memory pressure (e.g. mid a long corpus/benchmark run with
// prettier and other WASM modules active), so the formatter read corrupted
// input and silently dropped content — a non-deterministic false data-loss
// signal. The native `.so` is correct (verified byte-for-byte from Python
// ctypes, which passes immovable `bytes`); the bug is in Deno's buffer path.
//
// The pointers come from persistent marshalling buffers created in `init()`
// and grown (source/result) only when a larger file arrives: taking a pointer
// with `Deno.UnsafePointer.of` externalizes the array's backing store, and V8
// never relocates an externalized backing store (probe-verified stable across
// forced full GCs, including for sub-64-byte arrays whose stores start
// on-heap). So each pointer is taken once per (re)allocation, warm calls do no
// per-call allocation or pointer-taking, and there is no per-call GC interplay
// left for the buffer bug to exploit. The original `buffer`-path corruption no
// longer reproduces on current Deno under synthetic pressure (50k+ calls,
// churn + ballast + resident WASM); the explicit-pointer path is retained as
// defense-in-depth, and the corpus compare independently self-verifies any
// SAFETY finding by re-running the native format (see
// `corpus_compare_format.ts`).
// Every entry point has the same C signature — `(source_ptr, source_len, goal,
// out_len, out_status) -> payload_ptr` — so the table is one shape per name,
// with no goal-aware variants to keep in step. `goal` is 0 = Module, 1 = Script;
// Svelte and CSS reject a non-zero code rather than ignoring it.
const ENTRY_POINT = {
	parameters: ['pointer', 'usize', 'u32', 'pointer', 'pointer'],
	result: 'pointer'
} as const;

const symbols = {
	tsv_parse_svelte: ENTRY_POINT,
	tsv_parse_internal_svelte: ENTRY_POINT,
	tsv_format_svelte: ENTRY_POINT,
	tsv_parse_typescript: ENTRY_POINT,
	tsv_parse_internal_typescript: ENTRY_POINT,
	tsv_format_typescript: ENTRY_POINT,
	tsv_parse_css: ENTRY_POINT,
	tsv_parse_internal_css: ENTRY_POINT,
	tsv_format_css: ENTRY_POINT,
	// no-locations parse (span-only wire) — svelte + typescript only (CSS emits no `loc`)
	tsv_parse_svelte_no_locations: ENTRY_POINT,
	tsv_parse_typescript_no_locations: ENTRY_POINT,
	tsv_free: {
		parameters: ['pointer', 'usize'],
		result: 'void'
	}
} as const;

/** `*out_status` after a call that produced its payload (`tsv_ffi`'s `TSV_STATUS_OK`). */
const STATUS_OK = 0;

/**
 * Written into `out_status` BEFORE every call, so an export that returns without
 * writing it fails loudly here instead of inheriting the previous call's verdict.
 *
 * The buffer is persistent (see `MarshalState`), and every value the native side
 * can legitimately write is small, so a stale slot otherwise reads as whatever the
 * last call said — `STATUS_OK` after any success. The reject probe covers the four
 * Svelte operations at init; this covers every export on every call, for one store.
 * `tsv_ffi`'s own `call_raw` test helper seeds `u32::MAX` for the same reason.
 */
const STATUS_UNWRITTEN = 0xffffffff;

/** The C-ABI goal codes (`tsv_ffi`'s `ffi_goal`). */
const GOAL_MODULE = 0;
const GOAL_SCRIPT = 1;

type FfiFn = (
	source: Deno.PointerValue,
	len: number | bigint,
	goal: number,
	out_len: Deno.PointerValue,
	out_status: Deno.PointerValue
) => Deno.PointerValue;
type LibSymbols = Deno.DynamicLibrary<typeof symbols>['symbols'];

/** Get the native library path based on platform.
 * Uses TSV_FFI_PROFILE env var to select cargo profile (default: "release").
 * The corpus comparison task sets this to "corpus" for panic recovery.
 */
export function get_library_path(): string {
	return ffi_library_path(Deno.env.get('TSV_FFI_PROFILE') ?? 'release');
}

/** Persistent marshalling buffers + their externalized pointers (see the `symbols` comment). */
interface MarshalState {
	/** Receives the output byte length; written by the native side through `out_len_ptr`. */
	out_len_buffer: BigUint64Array;
	out_len_ptr: Deno.PointerValue;
	/**
	 * Receives the call's verdict (`STATUS_OK` or not), written through
	 * `out_status_ptr` alongside `out_len`. This — never the payload's shape — is
	 * what tells a refusal from a formatted file; see `lib/reject_probe.ts`.
	 *
	 * Persistent like the rest, so `call_ffi` seeds `STATUS_UNWRITTEN` before every
	 * call rather than reading whatever the last one left.
	 */
	out_status_buffer: Uint32Array;
	out_status_ptr: Deno.PointerValue;
	/** Grow-only UTF-8 staging for the source; re-pointed only on growth. */
	source_buffer: Uint8Array;
	source_ptr: Deno.PointerValue;
	/** Grow-only staging for the result copy-out (no pointer needed — `copyInto` takes the view). */
	result_buffer: Uint8Array;
}

/**
 * The per-language symbol tables, resolved ONCE in `init()`.
 *
 * These were getters returning a fresh object literal, so every timed call
 * allocated one — harness-side allocation charged to whichever row it sat under,
 * which belongs to no impl. Same reason `lib/canonical.ts` hoists its prettier
 * plugins array out of the per-call path.
 */
interface FfiTables {
	parse: Record<Language, FfiFn>;
	parse_internal: Record<Language, FfiFn>;
	/** Span-only wire — svelte + typescript only (CSS emits no `loc`). */
	parse_no_locations: Partial<Record<Language, FfiFn>>;
	format: Record<Language, FfiFn>;
}

/** Grow-only sizing: double `current` until it holds `needed`. */
const next_capacity = (needed: number, current: number): number => {
	let cap = current;
	while (cap < needed) cap *= 2;
	return cap;
};

const INITIAL_BUFFER_CAPACITY = 1 << 16;

export class NativeImplementation extends BaseImplementation {
	private _lib: Deno.DynamicLibrary<typeof symbols> | null = null;
	private _marshal: MarshalState | null = null;
	private _tables: FfiTables | null = null;
	private encoder = new TextEncoder();
	private decoder = new TextDecoder();

	readonly parse_languages = LANGUAGES;
	readonly format_languages = LANGUAGES;

	/** Get initialized library or throw */
	private get lib(): Deno.DynamicLibrary<typeof symbols> {
		if (!this._lib) throw new Error('Native library not initialized');
		return this._lib;
	}

	/** Get symbols with proper typing */
	private get symbols(): LibSymbols {
		return this.lib.symbols;
	}

	/** The per-language symbol tables, or throw if `init()` hasn't run. */
	private get tables(): FfiTables {
		if (!this._tables) throw new Error('Native library not initialized');
		return this._tables;
	}

	async init(): Promise<void> {
		const lib_path = get_library_path();

		const profile = Deno.env.get('TSV_FFI_PROFILE') ?? 'release';
		try {
			await Deno.stat(lib_path);
		} catch {
			throw new Error(
				`Native library not found at ${lib_path}. ` +
					`Run 'cargo build -p tsv_ffi --${
						profile === 'release' ? 'release' : `profile ${profile}`
					}' first.`
			);
		}

		this._lib = Deno.dlopen(lib_path, symbols);

		// Explicit `ArrayBuffer` backing so the stores are materialized off-heap
		// up front; the `UnsafePointer.of` calls externalize them, after which V8
		// never relocates them (see the `symbols` comment). The buffers live on
		// the instance, so they stay trivially reachable across every call.
		const out_len_buffer = new BigUint64Array(new ArrayBuffer(8));
		const out_status_buffer = new Uint32Array(new ArrayBuffer(4));
		const source_buffer = new Uint8Array(new ArrayBuffer(INITIAL_BUFFER_CAPACITY));
		this._marshal = {
			out_len_buffer,
			out_len_ptr: Deno.UnsafePointer.of(out_len_buffer),
			out_status_buffer,
			out_status_ptr: Deno.UnsafePointer.of(out_status_buffer),
			source_buffer,
			source_ptr: Deno.UnsafePointer.of(source_buffer),
			result_buffer: new Uint8Array(new ArrayBuffer(INITIAL_BUFFER_CAPACITY))
		};

		// Resolve every symbol table once — see `FfiTables`.
		this._tables = {
			parse: {
				svelte: this.symbols.tsv_parse_svelte as FfiFn,
				typescript: this.symbols.tsv_parse_typescript as FfiFn,
				css: this.symbols.tsv_parse_css as FfiFn
			},
			parse_internal: {
				svelte: this.symbols.tsv_parse_internal_svelte as FfiFn,
				typescript: this.symbols.tsv_parse_internal_typescript as FfiFn,
				css: this.symbols.tsv_parse_internal_css as FfiFn
			},
			parse_no_locations: {
				svelte: this.symbols.tsv_parse_svelte_no_locations as FfiFn,
				typescript: this.symbols.tsv_parse_typescript_no_locations as FfiFn
			},
			format: {
				svelte: this.symbols.tsv_format_svelte as FfiFn,
				typescript: this.symbols.tsv_format_typescript as FfiFn,
				css: this.symbols.tsv_format_css as FfiFn
			}
		};

		// This binding returns a payload either way, so the `out_status` word is the
		// only thing that tells a refusal from a formatted file. Prove it still
		// fires — see `lib/reject_probe.ts`.
		assert_binding_reports_rejection('tsv (FFI)', this);
	}

	/**
	 * Drive one entry point at `goal` and return its payload, throwing when the
	 * call reported an error.
	 *
	 * The verdict comes from `out_status`, read as one typed-array load beside the
	 * length that is already read here. It used to come from a `startsWith('{"error"')`
	 * test on the decoded payload — sound only because tsv normalizes strings to
	 * single quotes, i.e. a correctness dependency on a STYLE setting, over a
	 * channel that carries arbitrary formatted source (`lib/reject_probe.ts`).
	 */
	private call_ffi(fn: FfiFn, source: string, goal: ParseGoal = 'module'): string {
		const m = this._marshal;
		if (!m) throw new Error('Native library not initialized');

		// Worst-case UTF-8 length is 3 bytes per UTF-16 code unit (astral chars
		// are 2 units → 4 bytes, still ≤ 3 per unit), so one capacity check
		// guarantees `encodeInto` consumes the whole source.
		const max_bytes = source.length * 3;
		if (max_bytes > m.source_buffer.length) {
			m.source_buffer = new Uint8Array(
				new ArrayBuffer(next_capacity(max_bytes, m.source_buffer.length))
			);
			m.source_ptr = Deno.UnsafePointer.of(m.source_buffer);
		}
		const { read, written } = this.encoder.encodeInto(source, m.source_buffer);
		if (read !== source.length) {
			throw new Error(`encodeInto consumed ${read} of ${source.length} source units`);
		}

		m.out_status_buffer[0] = STATUS_UNWRITTEN;
		const result_ptr = fn(
			m.source_ptr,
			written,
			goal === 'script' ? GOAL_SCRIPT : GOAL_MODULE,
			m.out_len_ptr,
			m.out_status_ptr
		);

		if (result_ptr === null) {
			throw new Error('FFI function returned null pointer');
		}

		const result_len = m.out_len_buffer[0];
		const result_byte_count = Number(result_len);
		if (result_byte_count > m.result_buffer.length) {
			m.result_buffer = new Uint8Array(
				new ArrayBuffer(next_capacity(result_byte_count, m.result_buffer.length))
			);
		}

		// Read the result into the staging buffer, then free the native allocation
		// (length stays bigint through the free call).
		const result_bytes = m.result_buffer.subarray(0, result_byte_count);
		new Deno.UnsafePointerView(result_ptr).copyInto(result_bytes);
		this.symbols.tsv_free(result_ptr, result_len);

		const result = this.decoder.decode(result_bytes);
		const status = m.out_status_buffer[0];
		if (status === STATUS_UNWRITTEN) {
			throw new Error(
				`tsv left out_status unwritten — the call's verdict is unknowable, so this ` +
					`row's coverage would be fabricated. See lib/reject_probe.ts.`
			);
		}
		if (status !== STATUS_OK) {
			// The error payload is `error_result`'s compact `serde_json` object.
			// Parsed only once the status has already said this is an error, so a
			// formatted file never reaches `JSON.parse` — the loose prefix test this
			// replaced ran one over a whole `{#if …}` file's output on every timed call.
			let parsed;
			try {
				parsed = JSON.parse(result);
			} catch {
				throw new Error(`tsv reported an error with an unparseable payload: ${result}`);
			}
			throw new Error(parsed.error ?? result);
		}
		return result;
	}

	// `goal_for` withholds the goal for svelte/css, which REJECT a script code
	// rather than ignoring it (`tsv_ffi`'s `ffi_goal`). One shared helper for all
	// three wrappers — see its doc in `lib/types.ts`.
	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		return JSON.parse(this.call_ffi(this.tables.parse[language], source, goal_for(language, goal)));
	}

	parse_internal(source: string, language: Language, goal?: ParseGoal): void {
		this.call_ffi(this.tables.parse_internal[language], source, goal_for(language, goal));
	}

	parse_no_locations(source: string, language: Language, goal?: ParseGoal): unknown {
		const fn = this.tables.parse_no_locations[language];
		if (!fn) throw new Error(`no-locations parse unsupported for ${language}`);
		return JSON.parse(this.call_ffi(fn, source, goal_for(language, goal)));
	}

	format(source: string, language: Language): string {
		return this.call_ffi(this.tables.format[language], source);
	}

	dispose(): void {
		if (this._lib) {
			this._lib.close();
			this._lib = null;
		}
		this._marshal = null;
		this._tables = null;
	}
}

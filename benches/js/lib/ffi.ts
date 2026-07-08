/**
 * FFI bindings to native tsv library
 *
 * Uses Deno.dlopen to call the Rust library directly for maximum performance.
 */

import { native_library_filename } from './runtime.ts';
import type { Language, ParseGoal, TsvImplementation } from './types.ts';

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
const symbols = {
	tsv_parse_svelte: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	tsv_parse_internal_svelte: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	tsv_format_svelte: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	tsv_parse_typescript: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	tsv_parse_internal_typescript: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	tsv_format_typescript: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	tsv_parse_css: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	tsv_parse_internal_css: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	// no-locations parse (span-only wire) — svelte + typescript only (CSS emits no `loc`)
	tsv_parse_svelte_no_locations: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	tsv_parse_typescript_no_locations: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	// goal-aware TS parse (extra `u32` goal: 0 = Module, 1 = Script) — the
	// conformance surface's test262 files
	tsv_parse_typescript_with_goal: {
		parameters: ['pointer', 'usize', 'u32', 'pointer'],
		result: 'pointer',
	},
	tsv_parse_typescript_no_locations_with_goal: {
		parameters: ['pointer', 'usize', 'u32', 'pointer'],
		result: 'pointer',
	},
	tsv_parse_internal_typescript_with_goal: {
		parameters: ['pointer', 'usize', 'u32', 'pointer'],
		result: 'pointer',
	},
	tsv_format_css: {
		parameters: ['pointer', 'usize', 'pointer'],
		result: 'pointer',
	},
	tsv_free: {
		parameters: ['pointer', 'usize'],
		result: 'void',
	},
} as const;

type FfiFn = (
	source: Deno.PointerValue,
	len: number | bigint,
	out_len: Deno.PointerValue,
) => Deno.PointerValue;
/** Goal-aware TS parse symbol: an extra `u32` goal (0 = Module, 1 = Script). */
type FfiGoalFn = (
	source: Deno.PointerValue,
	len: number | bigint,
	goal: number,
	out_len: Deno.PointerValue,
) => Deno.PointerValue;
type LibSymbols = Deno.DynamicLibrary<typeof symbols>['symbols'];

/** Get the native library path based on platform.
 * Uses TSV_FFI_PROFILE env var to select cargo profile (default: "release").
 * The corpus comparison task sets this to "corpus" for panic recovery.
 */
export function get_library_path(): string {
	const profile = Deno.env.get('TSV_FFI_PROFILE') ?? 'release';
	const target_dir = new URL('../../../target', import.meta.url).pathname;
	return `${target_dir}/${profile}/${native_library_filename('tsv_ffi')}`;
}

/** Persistent marshalling buffers + their externalized pointers (see the `symbols` comment). */
interface MarshalState {
	/** Receives the output byte length; written by the native side through `out_len_ptr`. */
	out_len_buffer: BigUint64Array;
	out_len_ptr: Deno.PointerValue;
	/** Grow-only UTF-8 staging for the source; re-pointed only on growth. */
	source_buffer: Uint8Array;
	source_ptr: Deno.PointerValue;
	/** Grow-only staging for the result copy-out (no pointer needed — `copyInto` takes the view). */
	result_buffer: Uint8Array;
}

/** Grow-only sizing: double `current` until it holds `needed`. */
const next_capacity = (needed: number, current: number): number => {
	let cap = current;
	while (cap < needed) cap *= 2;
	return cap;
};

const INITIAL_BUFFER_CAPACITY = 1 << 16;

export class NativeImplementation implements TsvImplementation {
	name = 'native' as const;
	private _lib: Deno.DynamicLibrary<typeof symbols> | null = null;
	private _marshal: MarshalState | null = null;
	private encoder = new TextEncoder();
	private decoder = new TextDecoder();

	/** Languages supported for parsing */
	static readonly PARSE_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	/** Languages supported for formatting */
	static readonly FORMAT_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	/** Get initialized library or throw */
	private get lib(): Deno.DynamicLibrary<typeof symbols> {
		if (!this._lib) throw new Error('Native library not initialized');
		return this._lib;
	}

	/** Get symbols with proper typing */
	private get symbols(): LibSymbols {
		return this.lib.symbols;
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
					}' first.`,
			);
		}

		this._lib = Deno.dlopen(lib_path, symbols);

		// Explicit `ArrayBuffer` backing so the stores are materialized off-heap
		// up front; the `UnsafePointer.of` calls externalize them, after which V8
		// never relocates them (see the `symbols` comment). The buffers live on
		// the instance, so they stay trivially reachable across every call.
		const out_len_buffer = new BigUint64Array(new ArrayBuffer(8));
		const source_buffer = new Uint8Array(new ArrayBuffer(INITIAL_BUFFER_CAPACITY));
		this._marshal = {
			out_len_buffer,
			out_len_ptr: Deno.UnsafePointer.of(out_len_buffer),
			source_buffer,
			source_ptr: Deno.UnsafePointer.of(source_buffer),
			result_buffer: new Uint8Array(new ArrayBuffer(INITIAL_BUFFER_CAPACITY)),
		};
	}

	private call_ffi(fn: FfiFn, source: string): string {
		const m = this._marshal;
		if (!m) throw new Error('Native library not initialized');

		// Worst-case UTF-8 length is 3 bytes per UTF-16 code unit (astral chars
		// are 2 units → 4 bytes, still ≤ 3 per unit), so one capacity check
		// guarantees `encodeInto` consumes the whole source.
		const max_bytes = source.length * 3;
		if (max_bytes > m.source_buffer.length) {
			m.source_buffer = new Uint8Array(
				new ArrayBuffer(next_capacity(max_bytes, m.source_buffer.length)),
			);
			m.source_ptr = Deno.UnsafePointer.of(m.source_buffer);
		}
		const {read, written} = this.encoder.encodeInto(source, m.source_buffer);
		if (read !== source.length) {
			throw new Error(`encodeInto consumed ${read} of ${source.length} source units`);
		}

		const result_ptr = fn(m.source_ptr, written, m.out_len_ptr);

		if (result_ptr === null) {
			throw new Error('FFI function returned null pointer');
		}

		const result_len = m.out_len_buffer[0];
		const result_byte_count = Number(result_len);
		if (result_byte_count > m.result_buffer.length) {
			m.result_buffer = new Uint8Array(
				new ArrayBuffer(next_capacity(result_byte_count, m.result_buffer.length)),
			);
		}

		// Read the result into the staging buffer, then free the native allocation
		// (length stays bigint through the free call).
		const result_bytes = m.result_buffer.subarray(0, result_byte_count);
		new Deno.UnsafePointerView(result_ptr).copyInto(result_bytes);
		this.symbols.tsv_free(result_ptr, result_len);

		return this.decoder.decode(result_bytes);
	}

	/**
	 * `call_ffi` for the goal-aware TS symbols (extra `u32` goal argument). A
	 * near-duplicate of `call_ffi` rather than a shared closure so the timed
	 * `call_ffi` path takes no per-call indirection; this variant is only reached
	 * from the coverage-only conformance preflight.
	 */
	private call_ffi_goal(fn: FfiGoalFn, source: string, goal: ParseGoal): string {
		const m = this._marshal;
		if (!m) throw new Error('Native library not initialized');

		const max_bytes = source.length * 3;
		if (max_bytes > m.source_buffer.length) {
			m.source_buffer = new Uint8Array(
				new ArrayBuffer(next_capacity(max_bytes, m.source_buffer.length)),
			);
			m.source_ptr = Deno.UnsafePointer.of(m.source_buffer);
		}
		const {read, written} = this.encoder.encodeInto(source, m.source_buffer);
		if (read !== source.length) {
			throw new Error(`encodeInto consumed ${read} of ${source.length} source units`);
		}

		const result_ptr = fn(m.source_ptr, written, goal === 'script' ? 1 : 0, m.out_len_ptr);
		if (result_ptr === null) {
			throw new Error('FFI function returned null pointer');
		}

		const result_len = m.out_len_buffer[0];
		const result_byte_count = Number(result_len);
		if (result_byte_count > m.result_buffer.length) {
			m.result_buffer = new Uint8Array(
				new ArrayBuffer(next_capacity(result_byte_count, m.result_buffer.length)),
			);
		}

		const result_bytes = m.result_buffer.subarray(0, result_byte_count);
		new Deno.UnsafePointerView(result_ptr).copyInto(result_bytes);
		this.symbols.tsv_free(result_ptr, result_len);

		return this.decoder.decode(result_bytes);
	}

	/** Check FFI result for error and throw if present */
	private check_error(result: string): void {
		// Error responses are JSON objects with an "error" key
		// Check prefix first to avoid JSON.parse overhead on success
		if (result.length > 0 && result[0] === '{') {
			let parsed;
			try {
				parsed = JSON.parse(result);
			} catch {
				// Not valid JSON, not an error response
				return;
			}
			if (parsed.error) {
				throw new Error(parsed.error);
			}
		}
	}

	/** Check if parsing is supported for this language */
	supports_parse_language(language: Language): boolean {
		return NativeImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supports_format_language(language: Language): boolean {
		return NativeImplementation.FORMAT_LANGUAGES.includes(language);
	}

	// Lookup tables for FFI functions by language
	private get parse_fns(): Record<Language, FfiFn> {
		return {
			svelte: this.symbols.tsv_parse_svelte as FfiFn,
			typescript: this.symbols.tsv_parse_typescript as FfiFn,
			css: this.symbols.tsv_parse_css as FfiFn,
		};
	}

	private get parse_internal_fns(): Record<Language, FfiFn> {
		return {
			svelte: this.symbols.tsv_parse_internal_svelte as FfiFn,
			typescript: this.symbols.tsv_parse_internal_typescript as FfiFn,
			css: this.symbols.tsv_parse_internal_css as FfiFn,
		};
	}

	// Span-only wire — svelte + typescript only (CSS has no `loc`).
	private get parse_no_locations_fns(): Partial<Record<Language, FfiFn>> {
		return {
			svelte: this.symbols.tsv_parse_svelte_no_locations as FfiFn,
			typescript: this.symbols.tsv_parse_typescript_no_locations as FfiFn,
		};
	}

	private get format_fns(): Record<Language, FfiFn> {
		return {
			svelte: this.symbols.tsv_format_svelte as FfiFn,
			typescript: this.symbols.tsv_format_typescript as FfiFn,
			css: this.symbols.tsv_format_css as FfiFn,
		};
	}

	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		const result = goal && language === 'typescript'
			? this.call_ffi_goal(this.symbols.tsv_parse_typescript_with_goal as FfiGoalFn, source, goal)
			: this.call_ffi(this.parse_fns[language], source);
		const parsed = JSON.parse(result);
		if (parsed.error) {
			throw new Error(parsed.error);
		}
		return parsed;
	}

	parse_internal(source: string, language: Language, goal?: ParseGoal): void {
		const result = goal && language === 'typescript'
			? this.call_ffi_goal(
				this.symbols.tsv_parse_internal_typescript_with_goal as FfiGoalFn,
				source,
				goal,
			)
			: this.call_ffi(this.parse_internal_fns[language], source);
		this.check_error(result);
	}

	parse_no_locations(source: string, language: Language, goal?: ParseGoal): unknown {
		let result: string;
		if (goal && language === 'typescript') {
			result = this.call_ffi_goal(
				this.symbols.tsv_parse_typescript_no_locations_with_goal as FfiGoalFn,
				source,
				goal,
			);
		} else {
			const fn = this.parse_no_locations_fns[language];
			if (!fn) throw new Error(`no-locations parse unsupported for ${language}`);
			result = this.call_ffi(fn, source);
		}
		const parsed = JSON.parse(result);
		if (parsed.error) {
			throw new Error(parsed.error);
		}
		return parsed;
	}

	format(source: string, language: Language): string {
		const result = this.call_ffi(this.format_fns[language], source);
		this.check_error(result);
		return result;
	}

	dispose(): void {
		if (this._lib) {
			this._lib.close();
			this._lib = null;
		}
		this._marshal = null;
	}
}

/**
 * FFI bindings to native tsv library
 *
 * Uses Deno.dlopen to call the Rust library directly for maximum performance.
 */

import type { Language, TsvImplementation } from './types.ts';

// FFI symbol definitions.
//
// The source and out-length arguments are passed as explicit `pointer`s
// (`Deno.UnsafePointer.of(...)` in `call_ffi`) rather than the `buffer`
// parameter type. Deno 2.8's `buffer` fast-call marshalling intermittently
// hands the native side a stale/wrong source pointer under memory pressure
// (e.g. mid a long corpus/benchmark run with prettier and other WASM modules
// active), so the formatter reads corrupted input and silently drops content —
// a non-deterministic false data-loss signal. The native `.so` is correct
// (verified byte-for-byte from
// Python ctypes, which passes immovable `bytes`); the bug is in Deno's buffer
// path. ArrayBuffer backing stores are off-heap and not relocated by GC, so an
// explicit `UnsafePointer.of` pointer is stable for the synchronous call — but
// only as long as the backing typed array stays reachable. `UnsafePointer.of`
// returns an opaque `PointerValue` that does NOT keep the source array alive,
// so V8 could otherwise collect it the moment the pointer is taken. `call_ffi`
// pins `source_bytes` with an explicit liveness read after the native call
// returns (`out_len_buffer` is already pinned by reading its result below).
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
type LibSymbols = Deno.DynamicLibrary<typeof symbols>['symbols'];

/** Get the native library path based on platform.
 * Uses TSV_FFI_PROFILE env var to select cargo profile (default: "release").
 * The corpus comparison task sets this to "corpus" for panic recovery.
 */
export function get_library_path(): string {
	const lib_name = Deno.build.os === 'linux'
		? 'libtsv_ffi.so'
		: Deno.build.os === 'darwin'
		? 'libtsv_ffi.dylib'
		: Deno.build.os === 'windows'
		? 'tsv_ffi.dll'
		: (() => {
			throw new Error(`Unsupported platform: ${Deno.build.os}`);
		})();

	const profile = Deno.env.get('TSV_FFI_PROFILE') ?? 'release';
	const target_dir = new URL('../../../target', import.meta.url).pathname;
	return `${target_dir}/${profile}/${lib_name}`;
}

export class NativeImplementation implements TsvImplementation {
	name = 'native' as const;
	private _lib: Deno.DynamicLibrary<typeof symbols> | null = null;
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
	}

	private call_ffi(fn: FfiFn, source: string): string {
		const source_bytes = this.encoder.encode(source);
		const out_len_buffer = new BigUint64Array(1);

		// Pass explicit pointers (see the `symbols` comment for why). `source_bytes`
		// and `out_len_buffer` must stay alive across the synchronous call.
		// `Deno.UnsafePointer.of(...)` returns an opaque `PointerValue` that does NOT
		// keep its backing typed array reachable, so we hold a named reference and add
		// an explicit liveness read below to pin `source_bytes` past the call.
		const source_ptr = Deno.UnsafePointer.of(source_bytes);
		const result_ptr = fn(
			source_ptr,
			source_bytes.length,
			Deno.UnsafePointer.of(out_len_buffer),
		);

		// Keep `source_bytes` provably reachable until AFTER the native call returns.
		// The condition is always false (a `Uint8Array`'s `byteLength` is never
		// negative), but the optimizer cannot prove that without reading the array, so
		// it forces `source_bytes` to stay live across the `fn(...)` call above — which
		// is exactly what defeats the GC-collection class of corruption. `out_len_buffer`
		// is pinned the same way by the result read below.
		if (source_bytes.byteLength < 0) throw new Error('unreachable');

		if (result_ptr === null) {
			throw new Error('FFI function returned null pointer');
		}

		const result_len = out_len_buffer[0];

		// Read the result
		const result_view = new Deno.UnsafePointerView(result_ptr);
		const result_bytes = new Uint8Array(Number(result_len));
		result_view.copyInto(result_bytes);

		// Free the allocated memory (keep as bigint throughout)
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

	parse(source: string, language: Language): unknown {
		const result = this.call_ffi(this.parse_fns[language], source);
		const parsed = JSON.parse(result);
		if (parsed.error) {
			throw new Error(parsed.error);
		}
		return parsed;
	}

	parse_internal(source: string, language: Language): void {
		const result = this.call_ffi(this.parse_internal_fns[language], source);
		this.check_error(result);
	}

	parse_no_locations(source: string, language: Language): unknown {
		const fn = this.parse_no_locations_fns[language];
		if (!fn) throw new Error(`no-locations parse unsupported for ${language}`);
		const parsed = JSON.parse(this.call_ffi(fn, source));
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
	}
}

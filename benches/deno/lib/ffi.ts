/**
 * FFI bindings to native tsv library
 *
 * Uses Deno.dlopen to call the Rust library directly for maximum performance.
 */

import type { Language, TsvImplementation } from './types.ts';

// FFI symbol definitions
const symbols = {
	tsv_parse_svelte: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_parse_internal_svelte: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_format_svelte: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_parse_typescript: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_parse_internal_typescript: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_format_typescript: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_parse_css: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_parse_internal_css: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_format_css: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_free: {
		parameters: ['pointer', 'usize'],
		result: 'void',
	},
} as const;

type FfiFn = (
	source: Uint8Array,
	len: number | bigint,
	outLen: BigUint64Array,
) => Deno.PointerValue;
type LibSymbols = Deno.DynamicLibrary<typeof symbols>['symbols'];

/** Get the native library path based on platform.
 * Checks `target/corpus/` first (built with panic=unwind for catch_unwind),
 * then falls back to `target/release/`.
 */
function getLibraryPath(): string {
	const libName =
		Deno.build.os === 'linux'
			? 'libtsv_ffi.so'
			: Deno.build.os === 'darwin'
				? 'libtsv_ffi.dylib'
				: Deno.build.os === 'windows'
					? 'tsv_ffi.dll'
					: (() => {
							throw new Error(`Unsupported platform: ${Deno.build.os}`);
						})();

	const targetDir = new URL('../../../target', import.meta.url).pathname;

	// Prefer corpus profile (panic=unwind, catches panics gracefully)
	const corpusPath = `${targetDir}/corpus/${libName}`;
	try {
		Deno.statSync(corpusPath);
		return corpusPath;
	} catch {
		// Fall back to release
	}

	return `${targetDir}/release/${libName}`;
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
		const libPath = getLibraryPath();

		try {
			await Deno.stat(libPath);
		} catch {
			throw new Error(
				`Native library not found at ${libPath}. ` +
					`Run 'cargo build -p tsv_ffi --release' first.`,
			);
		}

		this._lib = Deno.dlopen(libPath, symbols);
	}

	private callFfi(fn: FfiFn, source: string): string {
		const sourceBytes = this.encoder.encode(source);
		const outLenBuffer = new BigUint64Array(1);

		const resultPtr = fn(sourceBytes, sourceBytes.length, outLenBuffer);

		if (resultPtr === null) {
			throw new Error('FFI function returned null pointer');
		}

		const resultLen = outLenBuffer[0];

		// Read the result
		const resultView = new Deno.UnsafePointerView(resultPtr);
		const resultBytes = new Uint8Array(Number(resultLen));
		resultView.copyInto(resultBytes);

		// Free the allocated memory (keep as bigint throughout)
		this.symbols.tsv_free(resultPtr, resultLen);

		return this.decoder.decode(resultBytes);
	}

	/** Check FFI result for error and throw if present */
	private checkError(result: string): void {
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
	supportsParseLanguage(language: Language): boolean {
		return NativeImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supportsFormatLanguage(language: Language): boolean {
		return NativeImplementation.FORMAT_LANGUAGES.includes(language);
	}

	// Lookup tables for FFI functions by language
	private get parseFns(): Record<Language, FfiFn> {
		return {
			svelte: this.symbols.tsv_parse_svelte as FfiFn,
			typescript: this.symbols.tsv_parse_typescript as FfiFn,
			css: this.symbols.tsv_parse_css as FfiFn,
		};
	}

	private get parseInternalFns(): Record<Language, FfiFn> {
		return {
			svelte: this.symbols.tsv_parse_internal_svelte as FfiFn,
			typescript: this.symbols.tsv_parse_internal_typescript as FfiFn,
			css: this.symbols.tsv_parse_internal_css as FfiFn,
		};
	}

	private get formatFns(): Record<Language, FfiFn> {
		return {
			svelte: this.symbols.tsv_format_svelte as FfiFn,
			typescript: this.symbols.tsv_format_typescript as FfiFn,
			css: this.symbols.tsv_format_css as FfiFn,
		};
	}

	parse(source: string, language: Language): unknown {
		const result = this.callFfi(this.parseFns[language], source);
		const parsed = JSON.parse(result);
		if (parsed.error) {
			throw new Error(parsed.error);
		}
		return parsed;
	}

	parseInternal(source: string, language: Language): void {
		const result = this.callFfi(this.parseInternalFns[language], source);
		this.checkError(result);
	}

	format(source: string, language: Language): string {
		const result = this.callFfi(this.formatFns[language], source);
		this.checkError(result);
		return result;
	}

	dispose(): void {
		if (this._lib) {
			this._lib.close();
			this._lib = null;
		}
	}
}

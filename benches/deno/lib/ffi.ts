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
	tsv_format_svelte: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_parse_typescript: {
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
	tsv_format_css: {
		parameters: ['buffer', 'usize', 'buffer'],
		result: 'pointer',
	},
	tsv_free: {
		parameters: ['pointer', 'usize'],
		result: 'void',
	},
} as const;

/** Get the native library path based on platform */
function getLibraryPath(): string {
	const base = new URL('../../../target/release', import.meta.url).pathname;

	switch (Deno.build.os) {
		case 'linux':
			return `${base}/libtsv_ffi.so`;
		case 'darwin':
			return `${base}/libtsv_ffi.dylib`;
		case 'windows':
			return `${base}/tsv_ffi.dll`;
		default:
			throw new Error(`Unsupported platform: ${Deno.build.os}`);
	}
}

export class NativeImplementation implements TsvImplementation {
	name = 'native' as const;
	private lib: Deno.DynamicLibrary<typeof symbols> | null = null;
	private encoder = new TextEncoder();
	private decoder = new TextDecoder();

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

		this.lib = Deno.dlopen(libPath, symbols);
	}

	// deno-lint-ignore no-explicit-any
	private callFfi(fn: any, source: string): string {
		if (!this.lib) throw new Error('Native library not initialized');

		const sourceBytes = this.encoder.encode(source);
		const outLenBuffer = new BigUint64Array(1);

		const resultPtr = fn(sourceBytes, sourceBytes.length, outLenBuffer);

		if (resultPtr === null) {
			throw new Error('FFI function returned null pointer');
		}

		const resultLen = Number(outLenBuffer[0]);

		// Read the result
		const resultView = new Deno.UnsafePointerView(resultPtr);
		const resultBytes = new Uint8Array(resultLen);
		resultView.copyInto(resultBytes);

		// Free the allocated memory
		this.lib.symbols.tsv_free(resultPtr, BigInt(resultLen));

		return this.decoder.decode(resultBytes);
	}

	parse(source: string, language: Language): unknown {
		if (!this.lib) throw new Error('Native library not initialized');

		let result: string;
		switch (language) {
			case 'svelte':
				result = this.callFfi(this.lib.symbols.tsv_parse_svelte, source);
				break;
			case 'typescript':
				result = this.callFfi(this.lib.symbols.tsv_parse_typescript, source);
				break;
			case 'css':
				result = this.callFfi(this.lib.symbols.tsv_parse_css, source);
				break;
		}

		const parsed = JSON.parse(result);
		if (parsed.error) {
			throw new Error(parsed.error);
		}
		return parsed;
	}

	format(source: string, language: Language): string {
		if (!this.lib) throw new Error('Native library not initialized');

		let result: string;
		switch (language) {
			case 'svelte':
				result = this.callFfi(this.lib.symbols.tsv_format_svelte, source);
				break;
			case 'typescript':
				result = this.callFfi(this.lib.symbols.tsv_format_typescript, source);
				break;
			case 'css':
				result = this.callFfi(this.lib.symbols.tsv_format_css, source);
				break;
		}

		// Check for error response
		if (result.startsWith('{"error":')) {
			const parsed = JSON.parse(result);
			throw new Error(parsed.error);
		}

		return result;
	}

	dispose(): void {
		if (this.lib) {
			this.lib.close();
			this.lib = null;
		}
	}
}

/**
 * TSV Benchmark Suite
 *
 * Compares parsing and formatting performance across three implementations:
 * - Canonical: prettier + svelte/compiler (JavaScript baseline)
 * - Native: tsv via FFI (Rust, maximum performance)
 * - WASM: tsv compiled to WebAssembly (portable, near-native)
 *
 * Run with: deno bench --allow-ffi --allow-read --allow-env benches/deno/bench.ts
 */

import { groupByLanguage, loadCorpus } from './lib/corpus.ts';
import { NativeImplementation } from './lib/ffi.ts';
import { WasmImplementation } from './lib/wasm.ts';
import { CanonicalImplementation } from './lib/canonical.ts';
import type { Language, SourceFile } from './lib/types.ts';

// ============================================================================
// Configuration
// ============================================================================

/** Skip implementations that aren't built yet */
const SKIP_MISSING_IMPLEMENTATIONS = true;

/** Limit files per language for faster iteration during development */
const MAX_FILES_PER_LANGUAGE = Deno.env.get('BENCH_LIMIT')
	? parseInt(Deno.env.get('BENCH_LIMIT')!)
	: undefined;

// ============================================================================
// Setup
// ============================================================================

console.log('Loading corpus...\n');
const { files } = await loadCorpus();
const byLanguage = groupByLanguage(files);

// Apply file limit if set
function limitFiles(files: SourceFile[]): SourceFile[] {
	if (MAX_FILES_PER_LANGUAGE && files.length > MAX_FILES_PER_LANGUAGE) {
		return files.slice(0, MAX_FILES_PER_LANGUAGE);
	}
	return files;
}

const svelteFiles = limitFiles(byLanguage.svelte);
const tsFiles = limitFiles(byLanguage.typescript);
const cssFiles = limitFiles(byLanguage.css);

console.log(`\nBenchmarking with:`);
console.log(`  Svelte: ${svelteFiles.length} files`);
console.log(`  TypeScript: ${tsFiles.length} files`);
console.log(`  CSS: ${cssFiles.length} files`);
console.log('');

// Initialize implementations
const canonical = new CanonicalImplementation();
const native = new NativeImplementation();
const wasm = new WasmImplementation();

let nativeAvailable = false;
let wasmAvailable = false;

console.log('Initializing implementations...');

try {
	await canonical.init();
	console.log('  ✓ Canonical (prettier + svelte/compiler)');
} catch (e) {
	console.error(`  ✗ Canonical: ${e}`);
	throw e; // Canonical is required
}

try {
	await native.init();
	nativeAvailable = true;
	console.log('  ✓ Native (FFI)');
} catch (e) {
	if (SKIP_MISSING_IMPLEMENTATIONS) {
		console.warn(`  ⚠ Native: ${e}`);
	} else {
		throw e;
	}
}

try {
	await wasm.init();
	wasmAvailable = true;
	console.log('  ✓ WASM');
} catch (e) {
	if (SKIP_MISSING_IMPLEMENTATIONS) {
		console.warn(`  ⚠ WASM: ${e}`);
	} else {
		throw e;
	}
}

console.log('');

// ============================================================================
// Benchmark Helpers
// ============================================================================

// Track skipped files for reporting
const skippedFiles: Map<string, Set<string>> = new Map();

function recordSkip(benchName: string, filePath: string): void {
	if (!skippedFiles.has(benchName)) {
		skippedFiles.set(benchName, new Set());
	}
	skippedFiles.get(benchName)!.add(filePath);
}

/** Process all files of a language with a given implementation */
function benchmarkParse(
	files: SourceFile[],
	language: Language,
	impl: { parse: (source: string, lang: Language) => unknown },
	benchName: string,
): void {
	for (const file of files) {
		try {
			impl.parse(file.content, language);
		} catch {
			recordSkip(benchName, file.path);
		}
	}
}

/** Process all files with native/wasm format */
function benchmarkFormat(
	files: SourceFile[],
	language: Language,
	impl: { format: (source: string, lang: Language) => string },
	benchName: string,
): void {
	for (const file of files) {
		try {
			impl.format(file.content, language);
		} catch {
			recordSkip(benchName, file.path);
		}
	}
}

/** Process all files with canonical async format */
async function benchmarkFormatAsync(
	files: SourceFile[],
	language: Language,
	impl: CanonicalImplementation,
	benchName: string,
): Promise<void> {
	for (const file of files) {
		try {
			await impl.formatAsync(file.content, language);
		} catch {
			recordSkip(benchName, file.path);
		}
	}
}

/** Report skipped files after benchmarks complete */
function reportSkippedFiles(): void {
	if (skippedFiles.size === 0) return;

	console.log('\n--- Skipped Files ---');
	for (const [benchName, files] of skippedFiles) {
		if (files.size > 0) {
			console.log(`\n${benchName}: ${files.size} file(s) skipped`);
			for (const file of [...files].slice(0, 5)) {
				console.log(`  - ${file}`);
			}
			if (files.size > 5) {
				console.log(`  ... and ${files.size - 5} more`);
			}
		}
	}
}

// ============================================================================
// Svelte Benchmarks
// ============================================================================

if (svelteFiles.length > 0) {
	Deno.bench({
		name: 'parse/svelte/canonical',
		group: 'parse-svelte',
		baseline: true,
		fn() {
			benchmarkParse(svelteFiles, 'svelte', canonical, 'parse/svelte/canonical');
		},
	});

	if (nativeAvailable) {
		Deno.bench({
			name: 'parse/svelte/native',
			group: 'parse-svelte',
			fn() {
				benchmarkParse(svelteFiles, 'svelte', native, 'parse/svelte/native');
			},
		});
	}

	if (wasmAvailable) {
		Deno.bench({
			name: 'parse/svelte/wasm',
			group: 'parse-svelte',
			fn() {
				benchmarkParse(svelteFiles, 'svelte', wasm, 'parse/svelte/wasm');
			},
		});
	}

	Deno.bench({
		name: 'format/svelte/canonical',
		group: 'format-svelte',
		baseline: true,
		async fn() {
			await benchmarkFormatAsync(svelteFiles, 'svelte', canonical, 'format/svelte/canonical');
		},
	});

	if (nativeAvailable) {
		Deno.bench({
			name: 'format/svelte/native',
			group: 'format-svelte',
			fn() {
				benchmarkFormat(svelteFiles, 'svelte', native, 'format/svelte/native');
			},
		});
	}

	if (wasmAvailable) {
		Deno.bench({
			name: 'format/svelte/wasm',
			group: 'format-svelte',
			fn() {
				benchmarkFormat(svelteFiles, 'svelte', wasm, 'format/svelte/wasm');
			},
		});
	}
}

// ============================================================================
// TypeScript Benchmarks
// ============================================================================

if (tsFiles.length > 0) {
	Deno.bench({
		name: 'parse/typescript/canonical',
		group: 'parse-typescript',
		baseline: true,
		fn() {
			benchmarkParse(tsFiles, 'typescript', canonical, 'parse/typescript/canonical');
		},
	});

	if (nativeAvailable) {
		Deno.bench({
			name: 'parse/typescript/native',
			group: 'parse-typescript',
			fn() {
				benchmarkParse(tsFiles, 'typescript', native, 'parse/typescript/native');
			},
		});
	}

	if (wasmAvailable) {
		Deno.bench({
			name: 'parse/typescript/wasm',
			group: 'parse-typescript',
			fn() {
				benchmarkParse(tsFiles, 'typescript', wasm, 'parse/typescript/wasm');
			},
		});
	}

	Deno.bench({
		name: 'format/typescript/canonical',
		group: 'format-typescript',
		baseline: true,
		async fn() {
			await benchmarkFormatAsync(tsFiles, 'typescript', canonical, 'format/typescript/canonical');
		},
	});

	if (nativeAvailable) {
		Deno.bench({
			name: 'format/typescript/native',
			group: 'format-typescript',
			fn() {
				benchmarkFormat(tsFiles, 'typescript', native, 'format/typescript/native');
			},
		});
	}

	if (wasmAvailable) {
		Deno.bench({
			name: 'format/typescript/wasm',
			group: 'format-typescript',
			fn() {
				benchmarkFormat(tsFiles, 'typescript', wasm, 'format/typescript/wasm');
			},
		});
	}
}

// ============================================================================
// CSS Benchmarks
// ============================================================================

if (cssFiles.length > 0) {
	// Note: No canonical CSS parser - our parser is the reference
	// Only benchmark format against prettier

	Deno.bench({
		name: 'format/css/canonical',
		group: 'format-css',
		baseline: true,
		async fn() {
			await benchmarkFormatAsync(cssFiles, 'css', canonical, 'format/css/canonical');
		},
	});

	if (nativeAvailable) {
		Deno.bench({
			name: 'format/css/native',
			group: 'format-css',
			fn() {
				benchmarkFormat(cssFiles, 'css', native, 'format/css/native');
			},
		});

		// Also benchmark CSS parsing (native vs wasm only)
		Deno.bench({
			name: 'parse/css/native',
			group: 'parse-css',
			baseline: true,
			fn() {
				benchmarkParse(cssFiles, 'css', native, 'parse/css/native');
			},
		});
	}

	if (wasmAvailable) {
		Deno.bench({
			name: 'format/css/wasm',
			group: 'format-css',
			fn() {
				benchmarkFormat(cssFiles, 'css', wasm, 'format/css/wasm');
			},
		});

		Deno.bench({
			name: 'parse/css/wasm',
			group: 'parse-css',
			fn() {
				benchmarkParse(cssFiles, 'css', wasm, 'parse/css/wasm');
			},
		});
	}
}

// ============================================================================
// Cleanup & Reporting
// ============================================================================

// Report skipped files after all benchmarks complete
// Note: This runs after Deno.bench registers tests but before they execute,
// so we use globalThis.addEventListener to run after benchmarks
globalThis.addEventListener('unload', () => {
	reportSkippedFiles();
});

// Note: Deno.bench doesn't have a cleanup hook, but the process will exit
// after benchmarks complete, releasing all resources.

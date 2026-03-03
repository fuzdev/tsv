/**
 * Binary/WASM size collection and reporting.
 *
 * Collects file sizes for each implementation's compiled binary:
 * - tsv: native (.so/.dylib/.dll) and WASM (.wasm)
 * - biome: WASM (.wasm) from npm cache
 * - oxc-parser: native (.node) and WASM (.wasm via binding-wasm32-wasi) from npm cache
 * - oxfmt: native (.node) from npm cache (no WASM variant)
 */

import type { AllVersions } from './versions.ts';

/** A collected binary size entry */
export interface BinarySize {
	/** Display label */
	label: string;
	/** Size in bytes */
	bytes: number;
	/** Binary kind for grouping comparisons */
	kind: 'wasm' | 'native';
}

/** Get the Deno npm cache base path */
function getDenoNpmCachePath(): string {
	const denoDir = Deno.env.get('DENO_DIR') ?? `${Deno.env.get('HOME')}/.cache/deno`;
	return `${denoDir}/npm/registry.npmjs.org`;
}

/** Get platform string for npm native binding packages (e.g., "linux-x64") */
function getNpmPlatform(): { os: string; arch: string } {
	const os = Deno.build.os === 'windows' ? 'win32' : Deno.build.os;
	const arch = Deno.build.arch === 'x86_64'
		? 'x64'
		: Deno.build.arch === 'aarch64'
		? 'arm64'
		: Deno.build.arch;
	return { os, arch };
}

/** Try to stat a file and return its size, or null if it doesn't exist */
async function fileSize(path: string): Promise<number | null> {
	try {
		const stat = await Deno.stat(path);
		return stat.size;
	} catch {
		return null;
	}
}

/** Find the first file with a given extension in a directory */
async function findFileByExtension(dir: string, ext: string): Promise<number | null> {
	try {
		for await (const entry of Deno.readDir(dir)) {
			if (entry.isFile && entry.name.endsWith(ext)) {
				return await fileSize(`${dir}/${entry.name}`);
			}
		}
	} catch {
		// Directory doesn't exist
	}
	return null;
}

/** Try multiple directory candidates for a native npm binding (.node file) */
async function findNpmNativeBinding(
	npmCache: string,
	scope: string,
	bindingPrefix: string,
	version: string,
): Promise<number | null> {
	const { os, arch } = getNpmPlatform();
	const platformBase = bindingPrefix ? `${bindingPrefix}-${os}-${arch}` : `${os}-${arch}`;

	// Try gnu variant first (more common on Linux), then musl, then bare (macOS/Windows)
	const candidates = [
		`${npmCache}/${scope}/${platformBase}-gnu/${version}`,
		`${npmCache}/${scope}/${platformBase}-musl/${version}`,
		`${npmCache}/${scope}/${platformBase}/${version}`,
	];

	for (const dir of candidates) {
		const size = await findFileByExtension(dir, '.node');
		if (size !== null) return size;
	}
	return null;
}

/**
 * Collect binary sizes for all implementations.
 *
 * Uses known paths for tsv binaries (relative to project root)
 * and Deno npm cache paths for npm packages.
 */
export async function collectBinarySizes(
	versions: AllVersions,
	options?: {
		hasNative?: boolean;
		hasWasm?: boolean;
		hasOxc?: boolean;
		hasBiome?: boolean;
	},
): Promise<BinarySize[]> {
	const sizes: BinarySize[] = [];
	const projectRoot = new URL('../../..', import.meta.url).pathname;
	const npmCache = getDenoNpmCachePath();

	// tsv native (FFI shared library)
	if (options?.hasNative !== false) {
		const ext = Deno.build.os === 'darwin' ? 'dylib' : Deno.build.os === 'windows' ? 'dll' : 'so';
		const prefix = Deno.build.os === 'windows' ? '' : 'lib';
		const path = `${projectRoot}/target/release/${prefix}tsv_ffi.${ext}`;
		const bytes = await fileSize(path);
		if (bytes !== null) {
			sizes.push({ label: 'tsv (native)', bytes, kind: 'native' });
		}
	}

	// tsv WASM
	if (options?.hasWasm !== false) {
		const path = `${projectRoot}/crates/tsv_wasm/pkg/deno/tsv_wasm_bg.wasm`;
		const bytes = await fileSize(path);
		if (bytes !== null) {
			sizes.push({ label: 'tsv_wasm', bytes, kind: 'wasm' });
		}
	}

	// biome WASM
	if (options?.hasBiome !== false) {
		const biomeDir = `${npmCache}/@biomejs/wasm-bundler/${versions.biome.wasm}`;
		const bytes = await findFileByExtension(biomeDir, '.wasm');
		if (bytes !== null) {
			sizes.push({ label: 'biome (wasm)', bytes, kind: 'wasm' });
		}
	}

	// oxc-parser native binding
	if (options?.hasOxc !== false) {
		const oxcBytes = await findNpmNativeBinding(
			npmCache,
			'@oxc-parser',
			'binding',
			versions.oxc['oxc-parser'],
		);
		if (oxcBytes !== null) {
			sizes.push({ label: 'oxc-parser (native)', bytes: oxcBytes, kind: 'native' });
		}

		// oxfmt native binding
		const oxfmtBytes = await findNpmNativeBinding(
			npmCache,
			'@oxfmt',
			'',
			versions.oxc.oxfmt,
		);
		if (oxfmtBytes !== null) {
			sizes.push({ label: 'oxfmt (native)', bytes: oxfmtBytes, kind: 'native' });
		}

		// oxc-parser WASM binding (@oxc-parser/binding-wasm32-wasi)
		const oxcWasmDir = `${npmCache}/@oxc-parser/binding-wasm32-wasi/${versions.oxc['oxc-parser']}`;
		const oxcWasmBytes = await findFileByExtension(oxcWasmDir, '.wasm');
		if (oxcWasmBytes !== null) {
			sizes.push({ label: 'oxc-parser (wasm)', bytes: oxcWasmBytes, kind: 'wasm' });
		}
	}

	return sizes;
}

/** Format bytes as human-readable size */
export function formatBytes(bytes: number): string {
	if (bytes >= 1_000_000) {
		return `${(bytes / 1_000_000).toFixed(1)} MB`;
	} else if (bytes >= 1_000) {
		return `${(bytes / 1_000).toFixed(1)} KB`;
	}
	return `${bytes} B`;
}

/** Build display entries grouped by kind, with combined oxc and ratios */
function buildDisplayEntries(sizes: BinarySize[]): {
	wasmEntries: { entry: BinarySize; ratio: number | null }[];
	nativeEntries: { entry: BinarySize; ratio: number | null }[];
} {
	const tsvNative = sizes.find((s) => s.label === 'tsv (native)');
	const tsvWasm = sizes.find((s) => s.label === 'tsv_wasm');

	const wasmSizes = sizes.filter((s) => s.kind === 'wasm');
	const nativeSizes = sizes.filter((s) => s.kind === 'native');

	// Build combined oxc-parser+oxfmt entry if both exist
	const oxcParser = nativeSizes.find((s) => s.label === 'oxc-parser (native)');
	const oxfmtEntry = nativeSizes.find((s) => s.label === 'oxfmt (native)');
	const combinedOxc: BinarySize | null = oxcParser && oxfmtEntry
		? {
			label: 'oxc-parser+oxfmt (native)',
			bytes: oxcParser.bytes + oxfmtEntry.bytes,
			kind: 'native',
		}
		: null;

	function ratioTo(entry: BinarySize, reference: BinarySize | undefined): number | null {
		if (!reference || entry === reference) return null;
		return entry.bytes / reference.bytes;
	}

	const wasmEntries = wasmSizes.map((entry) => ({ entry, ratio: ratioTo(entry, tsvWasm) }));

	const nativeEntries: { entry: BinarySize; ratio: number | null }[] = [];
	for (const entry of nativeSizes) {
		nativeEntries.push({ entry, ratio: ratioTo(entry, tsvNative) });
		if (entry === tsvNative && combinedOxc) {
			nativeEntries.push({ entry: combinedOxc, ratio: ratioTo(combinedOxc, tsvNative) });
		}
	}

	return { wasmEntries, nativeEntries };
}

/** Generate binary size comparison report (plain text) */
export function generateBinarySizeReport(sizes: BinarySize[]): string | null {
	if (sizes.length === 0) return null;

	const { wasmEntries, nativeEntries } = buildDisplayEntries(sizes);

	// Find longest label for padding
	const allEntries = [...wasmEntries, ...nativeEntries];
	const maxLabelLen = Math.max(...allEntries.map((e) => e.entry.label.length));

	function formatEntry(entry: BinarySize, ratio: number | null): string {
		const sizeStr = formatBytes(entry.bytes).padStart(10);
		const ratioStr = ratio !== null ? `  (${ratio.toFixed(1)}x tsv)` : '';
		return `  ${entry.label.padEnd(maxLabelLen)} ${sizeStr}${ratioStr}`;
	}

	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('BINARY SIZES:');

	if (wasmEntries.length > 0) {
		lines.push('');
		lines.push('  WASM modules:');
		for (const { entry, ratio } of wasmEntries) {
			lines.push('  ' + formatEntry(entry, ratio));
		}
	}

	if (nativeEntries.length > 0) {
		lines.push('');
		lines.push('  Native binaries:');
		for (const { entry, ratio } of nativeEntries) {
			lines.push('  ' + formatEntry(entry, ratio));
		}
	}

	return lines.join('\n');
}

/** Generate binary size comparison report (markdown table) */
export function generateBinarySizeMarkdown(sizes: BinarySize[]): string | null {
	if (sizes.length === 0) return null;

	const { wasmEntries, nativeEntries } = buildDisplayEntries(sizes);

	const lines: string[] = [];
	lines.push('## Binary Sizes\n');
	lines.push('| Binary | Size | vs tsv |');
	lines.push('| --- | ---: | ---: |');

	function addEntries(entries: { entry: BinarySize; ratio: number | null }[]): void {
		for (const { entry, ratio } of entries) {
			const ratioStr = ratio !== null ? `${ratio.toFixed(1)}x` : '-';
			lines.push(`| ${entry.label} | ${formatBytes(entry.bytes)} | ${ratioStr} |`);
		}
	}

	addEntries(wasmEntries);
	addEntries(nativeEntries);

	return lines.join('\n');
}

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
	/** Raw on-disk size in bytes */
	bytes: number;
	/**
	 * Gzipped size in bytes (approximates wire size for npm tarballs).
	 * `null` if `gzip` wasn't available on PATH or the file couldn't be read.
	 * Uses `gzip -c` (system default level), matching `scripts/publish_patch_npm.ts`.
	 */
	gzip_bytes: number | null;
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

/**
 * Return the gzipped size of a file, or `null` if gzip isn't available or
 * the file can't be read. Shells out to `gzip -c` (system default level) so
 * the number matches what `publish_patch_npm.ts` reports — Deno's
 * CompressionStream uses a different default level and runs ~2% high.
 */
async function gzipSize(path: string): Promise<number | null> {
	try {
		const output = await new Deno.Command('gzip', {
			args: ['-c', path],
			stdout: 'piped',
			stderr: 'null',
		}).output();
		if (!output.success) return null;
		return output.stdout.length;
	} catch {
		// Subprocess failed to spawn — gzip not on PATH (likely Windows without WSL).
		return null;
	}
}

/** Add an entry to `out` for `path` if the file exists; defer gzip to the caller. */
async function pushSize(
	out: { entry: Omit<BinarySize, 'gzip_bytes'>; path: string }[],
	label: string,
	kind: 'wasm' | 'native',
	path: string,
): Promise<void> {
	const bytes = await fileSize(path);
	if (bytes !== null) out.push({ entry: { label, bytes, kind }, path });
}

/** Resolve the first existing file (by extension) under any of the candidate dirs. */
async function resolveFirst(
	dirs: string[],
	ext: string,
): Promise<{ path: string; bytes: number } | null> {
	for (const dir of dirs) {
		try {
			for await (const e of Deno.readDir(dir)) {
				if (e.isFile && e.name.endsWith(ext)) {
					const path = `${dir}/${e.name}`;
					const bytes = await fileSize(path);
					if (bytes !== null) return { path, bytes };
				}
			}
		} catch {
			// directory missing — try next candidate
		}
	}
	return null;
}

/**
 * Collect binary sizes for all implementations.
 *
 * Uses known paths for tsv binaries (relative to project root)
 * and Deno npm cache paths for npm packages. Computes gzipped size
 * alongside raw size; gzip is shelled out and parallelized across all
 * collected entries, so adding it costs roughly the slowest single
 * compression (biome's 35 MB dominates).
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
	const projectRoot = new URL('../../..', import.meta.url).pathname;
	const npmCache = getDenoNpmCachePath();

	// Stage 1: collect (label, kind, path) for everything that exists.
	const staged: { entry: Omit<BinarySize, 'gzip_bytes'>; path: string }[] = [];

	// tsv native (FFI shared library)
	if (options?.hasNative !== false) {
		const ext = Deno.build.os === 'darwin' ? 'dylib' : Deno.build.os === 'windows' ? 'dll' : 'so';
		const prefix = Deno.build.os === 'windows' ? '' : 'lib';
		await pushSize(
			staged,
			'tsv (native)',
			'native',
			`${projectRoot}/target/release/${prefix}tsv_ffi.${ext}`,
		);
	}

	// tsv WASM — two builds from one crate via the `ast` feature:
	// pkg/deno (format-only, @fuzdev/tsv_fmt) and pkg/deno-parse
	// (parse + format, @fuzdev/tsv_parse).
	if (options?.hasWasm !== false) {
		await pushSize(
			staged,
			'tsv_fmt (wasm)',
			'wasm',
			`${projectRoot}/crates/tsv_wasm/pkg/deno/tsv_wasm_bg.wasm`,
		);
		await pushSize(
			staged,
			'tsv_parse (wasm)',
			'wasm',
			`${projectRoot}/crates/tsv_wasm/pkg/deno-parse/tsv_wasm_bg.wasm`,
		);
	}

	// biome WASM
	if (options?.hasBiome !== false) {
		const biomeDir = `${npmCache}/@biomejs/wasm-bundler/${versions.biome.wasm}`;
		const found = await resolveFirst([biomeDir], '.wasm');
		if (found !== null) {
			staged.push({ entry: { label: 'biome (wasm)', bytes: found.bytes, kind: 'wasm' }, path: found.path });
		}
	}

	// oxc-parser + oxfmt
	if (options?.hasOxc !== false) {
		const { os, arch } = getNpmPlatform();
		const oxcVer = versions.oxc['oxc-parser'];
		const oxfmtVer = versions.oxc.oxfmt;

		const oxcDirs = [
			`${npmCache}/@oxc-parser/binding-${os}-${arch}-gnu/${oxcVer}`,
			`${npmCache}/@oxc-parser/binding-${os}-${arch}-musl/${oxcVer}`,
			`${npmCache}/@oxc-parser/binding-${os}-${arch}/${oxcVer}`,
		];
		const oxcFound = await resolveFirst(oxcDirs, '.node');
		if (oxcFound !== null) {
			staged.push({ entry: { label: 'oxc-parser (native)', bytes: oxcFound.bytes, kind: 'native' }, path: oxcFound.path });
		}

		// oxfmt native binding (0.50.0+: @oxfmt/binding-{platform}; pre-0.49: @oxfmt/{platform}).
		const oxfmtDirs = [
			`${npmCache}/@oxfmt/binding-${os}-${arch}-gnu/${oxfmtVer}`,
			`${npmCache}/@oxfmt/binding-${os}-${arch}-musl/${oxfmtVer}`,
			`${npmCache}/@oxfmt/binding-${os}-${arch}/${oxfmtVer}`,
		];
		const oxfmtFound = await resolveFirst(oxfmtDirs, '.node');
		if (oxfmtFound !== null) {
			staged.push({ entry: { label: 'oxfmt (native)', bytes: oxfmtFound.bytes, kind: 'native' }, path: oxfmtFound.path });
		}

		// oxc-parser WASM binding (@oxc-parser/binding-wasm32-wasi)
		const oxcWasmFound = await resolveFirst(
			[`${npmCache}/@oxc-parser/binding-wasm32-wasi/${oxcVer}`],
			'.wasm',
		);
		if (oxcWasmFound !== null) {
			staged.push({ entry: { label: 'oxc-parser (wasm)', bytes: oxcWasmFound.bytes, kind: 'wasm' }, path: oxcWasmFound.path });
		}
	}

	// Stage 2: gzip every collected file in parallel.
	const gzipped = await Promise.all(staged.map((s) => gzipSize(s.path)));

	return staged.map(({ entry }, i) => ({ ...entry, gzip_bytes: gzipped[i] }));
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

/** Display row: an entry plus ratios vs tsv on raw and gzipped bytes. */
interface DisplayRow {
	entry: BinarySize;
	ratio: number | null;
	gzipRatio: number | null;
}

/** Build display entries grouped by kind, with combined oxc and ratios */
function buildDisplayEntries(sizes: BinarySize[]): {
	wasmEntries: DisplayRow[];
	nativeEntries: DisplayRow[];
} {
	const tsvNative = sizes.find((s) => s.label === 'tsv (native)');
	const tsvWasm = sizes.find((s) => s.label === 'tsv_fmt (wasm)');

	const wasmSizes = sizes.filter((s) => s.kind === 'wasm');
	const nativeSizes = sizes.filter((s) => s.kind === 'native');

	// Build combined oxc-parser+oxfmt entry if both exist. Combined gzip is
	// the sum of the parts' gzipped sizes; that overstates wire size slightly
	// (two streams don't share a dictionary) but matches how npm ships them
	// — each binding is its own tarball.
	const oxcParser = nativeSizes.find((s) => s.label === 'oxc-parser (native)');
	const oxfmtEntry = nativeSizes.find((s) => s.label === 'oxfmt (native)');
	const combinedOxc: BinarySize | null = oxcParser && oxfmtEntry
		? {
			label: 'oxc-parser+oxfmt (native)',
			bytes: oxcParser.bytes + oxfmtEntry.bytes,
			gzip_bytes: oxcParser.gzip_bytes !== null && oxfmtEntry.gzip_bytes !== null
				? oxcParser.gzip_bytes + oxfmtEntry.gzip_bytes
				: null,
			kind: 'native',
		}
		: null;

	function ratioTo(entry: BinarySize, reference: BinarySize | undefined): number | null {
		if (!reference || entry === reference) return null;
		return entry.bytes / reference.bytes;
	}

	function gzipRatioTo(entry: BinarySize, reference: BinarySize | undefined): number | null {
		if (!reference || entry === reference) return null;
		if (entry.gzip_bytes === null || reference.gzip_bytes === null) return null;
		return entry.gzip_bytes / reference.gzip_bytes;
	}

	function row(entry: BinarySize, reference: BinarySize | undefined): DisplayRow {
		return { entry, ratio: ratioTo(entry, reference), gzipRatio: gzipRatioTo(entry, reference) };
	}

	const wasmEntries = wasmSizes.map((entry) => row(entry, tsvWasm));

	const nativeEntries: DisplayRow[] = [];
	for (const entry of nativeSizes) {
		nativeEntries.push(row(entry, tsvNative));
		if (entry === tsvNative && combinedOxc) {
			nativeEntries.push(row(combinedOxc, tsvNative));
		}
	}

	return { wasmEntries, nativeEntries };
}

/** Format a gzipped byte count or fall back to em-dash when unavailable. */
function formatGzipBytes(bytes: number | null): string {
	return bytes === null ? '—' : formatBytes(bytes);
}

/** Format a ratio (e.g. "1.3x"), or em-dash when missing/self. */
function formatRatio(ratio: number | null): string {
	return ratio === null ? '—' : `${ratio.toFixed(1)}x`;
}

/** True if any row has a gzipped size — i.e., gzip ran successfully somewhere. */
function anyGzipped(rows: DisplayRow[]): boolean {
	return rows.some((r) => r.entry.gzip_bytes !== null);
}

/** Generate binary size comparison report (plain text) */
export function generateBinarySizeReport(sizes: BinarySize[]): string | null {
	if (sizes.length === 0) return null;

	const { wasmEntries, nativeEntries } = buildDisplayEntries(sizes);
	const allRows = [...wasmEntries, ...nativeEntries];
	const showGzip = anyGzipped(allRows);

	const maxLabelLen = Math.max(...allRows.map((r) => r.entry.label.length));

	function formatRow({ entry, ratio, gzipRatio }: DisplayRow): string {
		const sizeStr = formatBytes(entry.bytes).padStart(10);
		const gzipStr = showGzip ? `  gz ${formatGzipBytes(entry.gzip_bytes).padStart(8)}` : '';
		const ratioStr = ratio !== null
			? `  (${ratio.toFixed(1)}x tsv${
				showGzip && gzipRatio !== null ? `, ${gzipRatio.toFixed(1)}x gz` : ''
			})`
			: '';
		return `  ${entry.label.padEnd(maxLabelLen)} ${sizeStr}${gzipStr}${ratioStr}`;
	}

	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('BINARY SIZES:');

	if (wasmEntries.length > 0) {
		lines.push('');
		lines.push('  WASM modules:');
		for (const r of wasmEntries) lines.push('  ' + formatRow(r));
	}

	if (nativeEntries.length > 0) {
		lines.push('');
		lines.push('  Native binaries:');
		for (const r of nativeEntries) lines.push('  ' + formatRow(r));
	}

	if (showGzip) {
		lines.push('');
		lines.push('  Gzipped column ≈ wire size for npm tarballs (`gzip -c`, system default level).');
	}

	return lines.join('\n');
}

/** Generate binary size comparison report (markdown table) */
export function generateBinarySizeMarkdown(sizes: BinarySize[]): string | null {
	if (sizes.length === 0) return null;

	const { wasmEntries, nativeEntries } = buildDisplayEntries(sizes);
	const showGzip = anyGzipped([...wasmEntries, ...nativeEntries]);

	const lines: string[] = [];
	lines.push('## Binary Sizes\n');
	if (showGzip) {
		lines.push('| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |');
		lines.push('| --- | ---: | ---: | ---: | ---: |');
	} else {
		lines.push('| Binary | Size | vs tsv |');
		lines.push('| --- | ---: | ---: |');
	}

	function addRows(rows: DisplayRow[]): void {
		for (const { entry, ratio, gzipRatio } of rows) {
			const cells = showGzip
				? [
					entry.label,
					formatBytes(entry.bytes),
					formatGzipBytes(entry.gzip_bytes),
					formatRatio(ratio),
					formatRatio(gzipRatio),
				]
				: [entry.label, formatBytes(entry.bytes), formatRatio(ratio)];
			lines.push(`| ${cells.join(' | ')} |`);
		}
	}

	addRows(wasmEntries);
	addRows(nativeEntries);

	if (showGzip) {
		lines.push('');
		lines.push(
			'_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._',
		);
	}

	return lines.join('\n');
}

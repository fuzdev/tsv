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
	 * Uses `gzip -c` (system default level), matching `scripts/patch_npm_package.ts`.
	 */
	gzip_bytes: number | null;
	/** Binary kind for grouping comparisons */
	kind: 'wasm' | 'native';
}

/** Get the Deno npm cache base path */
function get_deno_npm_cache_path(): string {
	const deno_dir = Deno.env.get('DENO_DIR') ?? `${Deno.env.get('HOME')}/.cache/deno`;
	return `${deno_dir}/npm/registry.npmjs.org`;
}

/** Get platform string for npm native binding packages (e.g., "linux-x64") */
function get_npm_platform(): { os: string; arch: string } {
	const os = Deno.build.os === 'windows' ? 'win32' : Deno.build.os;
	const arch = Deno.build.arch === 'x86_64'
		? 'x64'
		: Deno.build.arch === 'aarch64'
		? 'arm64'
		: Deno.build.arch;
	return { os, arch };
}

/** Try to stat a file and return its size, or null if it doesn't exist */
async function file_size(path: string): Promise<number | null> {
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
 * the number matches what `patch_npm_package.ts` reports — Deno's
 * CompressionStream uses a different default level and runs ~2% high.
 */
async function gzip_size(path: string): Promise<number | null> {
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

/** A pre-gzip staged entry: known label/bytes/kind plus the path to compress. */
type StagedEntry = { entry: Omit<BinarySize, 'gzip_bytes'>; path: string };

/** Add an entry to `out` for `path` if the file exists; defer gzip to the caller. */
async function push_size(
	out: StagedEntry[],
	label: string,
	kind: 'wasm' | 'native',
	path: string,
): Promise<void> {
	const bytes = await file_size(path);
	if (bytes !== null) out.push({ entry: { label, bytes, kind }, path });
}

/** Resolve the first existing file (by extension) under any of the candidate dirs. */
async function resolve_first(
	dirs: string[],
	ext: string,
): Promise<{ path: string; bytes: number } | null> {
	for (const dir of dirs) {
		try {
			for await (const e of Deno.readDir(dir)) {
				if (e.isFile && e.name.endsWith(ext)) {
					const path = `${dir}/${e.name}`;
					const bytes = await file_size(path);
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
export async function collect_binary_sizes(
	versions: AllVersions,
	options?: {
		has_native?: boolean;
		has_wasm?: boolean;
		has_oxc?: boolean;
		has_biome?: boolean;
	},
): Promise<BinarySize[]> {
	const project_root = new URL('../../..', import.meta.url).pathname;
	const npm_cache = get_deno_npm_cache_path();

	// Stage 1: collect (label, kind, path) for everything that exists.
	const staged: StagedEntry[] = [];

	// tsv native (FFI shared library)
	if (options?.has_native !== false) {
		const ext = Deno.build.os === 'darwin' ? 'dylib' : Deno.build.os === 'windows' ? 'dll' : 'so';
		const prefix = Deno.build.os === 'windows' ? '' : 'lib';
		await push_size(
			staged,
			'tsv',
			'native',
			`${project_root}/target/release/${prefix}tsv_ffi.${ext}`,
		);
	}

	// tsv WASM — three builds from one crate via the `format`/`parse` features:
	// pkg/format/deno (format-only, @fuzdev/tsv_format_wasm), pkg/parse/deno
	// (parse-only, @fuzdev/tsv_parse_wasm), and pkg/all/deno (both,
	// @fuzdev/tsv_wasm — the bundle the bench executes).
	if (options?.has_wasm !== false) {
		await push_size(
			staged,
			'tsv_format_wasm',
			'wasm',
			`${project_root}/crates/tsv_wasm/pkg/format/deno/tsv_wasm_bg.wasm`,
		);
		await push_size(
			staged,
			'tsv_parse_wasm',
			'wasm',
			`${project_root}/crates/tsv_wasm/pkg/parse/deno/tsv_wasm_bg.wasm`,
		);
		await push_size(
			staged,
			'tsv_wasm',
			'wasm',
			`${project_root}/crates/tsv_wasm/pkg/all/deno/tsv_wasm_bg.wasm`,
		);
	}

	// biome WASM
	if (options?.has_biome !== false) {
		const biome_dir = `${npm_cache}/@biomejs/wasm-bundler/${versions.biome.wasm}`;
		const found = await resolve_first([biome_dir], '.wasm');
		if (found !== null) {
			staged.push({
				entry: { label: 'biome (wasm)', bytes: found.bytes, kind: 'wasm' },
				path: found.path,
			});
		}
	}

	// oxc-parser + oxfmt
	if (options?.has_oxc !== false) {
		const { os, arch } = get_npm_platform();
		const oxc_ver = versions.oxc['oxc-parser'];
		const oxfmt_ver = versions.oxc.oxfmt;

		const oxc_dirs = [
			`${npm_cache}/@oxc-parser/binding-${os}-${arch}-gnu/${oxc_ver}`,
			`${npm_cache}/@oxc-parser/binding-${os}-${arch}-musl/${oxc_ver}`,
			`${npm_cache}/@oxc-parser/binding-${os}-${arch}/${oxc_ver}`,
		];
		const oxc_found = await resolve_first(oxc_dirs, '.node');
		if (oxc_found !== null) {
			staged.push({
				entry: { label: 'oxc-parser (native)', bytes: oxc_found.bytes, kind: 'native' },
				path: oxc_found.path,
			});
		}

		// oxfmt native binding (0.50.0+: @oxfmt/binding-{platform}; pre-0.49: @oxfmt/{platform}).
		const oxfmt_dirs = [
			`${npm_cache}/@oxfmt/binding-${os}-${arch}-gnu/${oxfmt_ver}`,
			`${npm_cache}/@oxfmt/binding-${os}-${arch}-musl/${oxfmt_ver}`,
			`${npm_cache}/@oxfmt/binding-${os}-${arch}/${oxfmt_ver}`,
		];
		const oxfmt_found = await resolve_first(oxfmt_dirs, '.node');
		if (oxfmt_found !== null) {
			staged.push({
				entry: { label: 'oxfmt (native)', bytes: oxfmt_found.bytes, kind: 'native' },
				path: oxfmt_found.path,
			});
		}

		// oxc-parser WASM binding (@oxc-parser/binding-wasm32-wasi)
		const oxc_wasm_found = await resolve_first(
			[`${npm_cache}/@oxc-parser/binding-wasm32-wasi/${oxc_ver}`],
			'.wasm',
		);
		if (oxc_wasm_found !== null) {
			staged.push({
				entry: { label: 'oxc-parser (wasm)', bytes: oxc_wasm_found.bytes, kind: 'wasm' },
				path: oxc_wasm_found.path,
			});
		}
	}

	// Stage 2: gzip every collected file in parallel.
	const gzipped = await Promise.all(staged.map((s) => gzip_size(s.path)));

	return staged.map(({ entry }, i) => ({ ...entry, gzip_bytes: gzipped[i] }));
}

/** Format bytes as human-readable size */
export function format_bytes(bytes: number): string {
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
	gzip_ratio: number | null;
}

/** Build display entries grouped by kind, with combined oxc and ratios */
function build_display_entries(sizes: BinarySize[]): {
	wasm_entries: DisplayRow[];
	native_entries: DisplayRow[];
} {
	const tsv_native = sizes.find((s) => s.label === 'tsv');
	// "vs tsv" wasm anchor: the flagship full build (`tsv_wasm`, the artifact
	// the bench executes), falling back to the format subset for reports
	// generated before shape v2 added the third build.
	const tsv_wasm = sizes.find((s) => s.label === 'tsv_wasm') ??
		sizes.find((s) => s.label === 'tsv_format_wasm');

	const wasm_sizes = sizes.filter((s) => s.kind === 'wasm');
	const native_sizes = sizes.filter((s) => s.kind === 'native');

	// Build combined oxc-parser+oxfmt entry if both exist. Combined gzip is
	// the sum of the parts' gzipped sizes; that overstates wire size slightly
	// (two streams don't share a dictionary) but matches how npm ships them
	// — each binding is its own tarball.
	const oxc_parser = native_sizes.find((s) => s.label === 'oxc-parser (native)');
	const oxfmt_entry = native_sizes.find((s) => s.label === 'oxfmt (native)');
	const combined_oxc: BinarySize | null = oxc_parser && oxfmt_entry
		? {
			label: 'oxc-parser+oxfmt (native)',
			bytes: oxc_parser.bytes + oxfmt_entry.bytes,
			gzip_bytes: oxc_parser.gzip_bytes !== null && oxfmt_entry.gzip_bytes !== null
				? oxc_parser.gzip_bytes + oxfmt_entry.gzip_bytes
				: null,
			kind: 'native',
		}
		: null;

	function ratio_to(entry: BinarySize, reference: BinarySize | undefined): number | null {
		if (!reference || entry === reference) return null;
		return entry.bytes / reference.bytes;
	}

	function gzip_ratio_to(entry: BinarySize, reference: BinarySize | undefined): number | null {
		if (!reference || entry === reference) return null;
		if (entry.gzip_bytes === null || reference.gzip_bytes === null) return null;
		return entry.gzip_bytes / reference.gzip_bytes;
	}

	function row(entry: BinarySize, reference: BinarySize | undefined): DisplayRow {
		return {
			entry,
			ratio: ratio_to(entry, reference),
			gzip_ratio: gzip_ratio_to(entry, reference),
		};
	}

	const wasm_entries = wasm_sizes.map((entry) => row(entry, tsv_wasm));

	const native_entries: DisplayRow[] = [];
	for (const entry of native_sizes) {
		native_entries.push(row(entry, tsv_native));
		if (entry === tsv_native && combined_oxc) {
			native_entries.push(row(combined_oxc, tsv_native));
		}
	}

	return { wasm_entries, native_entries };
}

/** Format a gzipped byte count or fall back to em-dash when unavailable. */
function format_gzip_bytes(bytes: number | null): string {
	return bytes === null ? '—' : format_bytes(bytes);
}

/** Format a ratio (e.g. "1.3x"), or em-dash when missing/self. */
function format_ratio(ratio: number | null): string {
	return ratio === null ? '—' : `${ratio.toFixed(1)}x`;
}

/** True if any row has a gzipped size — i.e., gzip ran successfully somewhere. */
function any_gzipped(rows: DisplayRow[]): boolean {
	return rows.some((r) => r.entry.gzip_bytes !== null);
}

/** Generate binary size comparison report (plain text) */
export function generate_binary_size_report(sizes: BinarySize[]): string | null {
	if (sizes.length === 0) return null;

	const { wasm_entries, native_entries } = build_display_entries(sizes);
	const all_rows = [...wasm_entries, ...native_entries];
	const show_gzip = any_gzipped(all_rows);

	const max_label_len = Math.max(...all_rows.map((r) => r.entry.label.length));

	function format_row({ entry, ratio, gzip_ratio }: DisplayRow): string {
		const size_str = format_bytes(entry.bytes).padStart(10);
		const gzip_str = show_gzip ? `  gz ${format_gzip_bytes(entry.gzip_bytes).padStart(8)}` : '';
		const ratio_str = ratio !== null
			? `  (${ratio.toFixed(1)}x tsv${
				show_gzip && gzip_ratio !== null ? `, ${gzip_ratio.toFixed(1)}x gz` : ''
			})`
			: '';
		return `  ${entry.label.padEnd(max_label_len)} ${size_str}${gzip_str}${ratio_str}`;
	}

	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('BINARY SIZES:');

	if (wasm_entries.length > 0) {
		lines.push('');
		lines.push('  WASM modules:');
		for (const r of wasm_entries) lines.push('  ' + format_row(r));
	}

	if (native_entries.length > 0) {
		lines.push('');
		lines.push('  Native binaries:');
		for (const r of native_entries) lines.push('  ' + format_row(r));
	}

	if (show_gzip) {
		lines.push('');
		lines.push('  Gzipped column ≈ wire size for npm tarballs (`gzip -c`, system default level).');
	}

	return lines.join('\n');
}

/** Generate binary size comparison report (markdown table) */
export function generate_binary_size_markdown(sizes: BinarySize[]): string | null {
	if (sizes.length === 0) return null;

	const { wasm_entries, native_entries } = build_display_entries(sizes);
	const show_gzip = any_gzipped([...wasm_entries, ...native_entries]);

	const lines: string[] = [];
	lines.push('## Binary Sizes\n');
	if (show_gzip) {
		lines.push('| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |');
		lines.push('| --- | ---: | ---: | ---: | ---: |');
	} else {
		lines.push('| Binary | Size | vs tsv |');
		lines.push('| --- | ---: | ---: |');
	}

	function add_rows(rows: DisplayRow[]): void {
		for (const { entry, ratio, gzip_ratio } of rows) {
			const cells = show_gzip
				? [
					entry.label,
					format_bytes(entry.bytes),
					format_gzip_bytes(entry.gzip_bytes),
					format_ratio(ratio),
					format_ratio(gzip_ratio),
				]
				: [entry.label, format_bytes(entry.bytes), format_ratio(ratio)];
			lines.push(`| ${cells.join(' | ')} |`);
		}
	}

	add_rows(wasm_entries);
	add_rows(native_entries);

	if (show_gzip) {
		lines.push('');
		lines.push(
			'_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._',
		);
	}

	return lines.join('\n');
}

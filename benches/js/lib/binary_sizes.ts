/**
 * Binary/WASM size collection and reporting.
 *
 * Collects file sizes for each implementation's compiled binary. Native-kind
 * rows are labeled by their binding mechanism: tsv ships an FFI shared library
 * AND an N-API addon; oxc-parser and oxfmt ship N-API `.node` bindings.
 * - tsv: FFI (.so/.dylib/.dll), N-API (.node), and WASM (.wasm)
 * - biome: WASM (.wasm) from node_modules
 * - oxc-parser: N-API (.node) and WASM (.wasm via binding-wasm32-wasi) from node_modules
 * - oxfmt: N-API (.node) from node_modules (no WASM variant)
 * - yuku-parser: N-API (.node) and WASM (.wasm) from node_modules
 *
 * Portable across runtimes: uses `node:` builtins (Deno supports them) and the
 * shared `runtime.ts` platform normalizer instead of `Deno.*`. The alternative
 * impls' bindings now live in the harness `node_modules` (flat, no version dir),
 * not the Deno npm cache — see benches/js/package.json.
 */

import { execFile } from 'node:child_process';
import { readdir, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { OXC_WASI_BINDING } from './check_node_modules.ts';
import type { ImplementationSet } from './implementations.ts';
import { current_arch, current_os, current_runtime, native_library_filename } from './runtime.ts';
import { rsvelte_binary_path } from './rsvelte.ts';

const exec_file = promisify(execFile);

/** Binary kind for grouping comparisons */
export type BinaryKind = 'wasm' | 'native';

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
	kind: BinaryKind;
}

/**
 * Display labels — also the identity keys for the ratio-anchor lookups in
 * `build_display_entries`, so the producer (`collect_binary_sizes`) and the
 * consumer must reference the same constant. Renaming a label here updates both.
 */
const LABELS = {
	tsv_ffi: 'tsv (ffi)',
	tsv_napi: 'tsv (napi)',
	tsv_format_ffi: 'tsv format (ffi)',
	tsv_parse_ffi: 'tsv parse (ffi)',
	tsv_format_wasm: 'tsv_format_wasm',
	tsv_parse_wasm: 'tsv_parse_wasm',
	tsv_wasm: 'tsv_wasm',
	biome_wasm: 'biome (wasm)',
	dprint_wasm: 'dprint (wasm)',
	oxc_parser_napi: 'oxc-parser (napi)',
	oxfmt_napi: 'oxfmt (napi)',
	oxc_parser_wasm: 'oxc-parser (wasm)',
	oxc_combined_napi: 'oxc-parser+oxfmt (napi)',
	yuku_parser_napi: 'yuku-parser (napi)',
	yuku_parser_wasm: 'yuku-parser (wasm)',
	malva_wasm: 'malva (wasm)',
	rsvelte_fmt_native: 'rsvelte-fmt (binary)',
	rsvelte_parse_napi: 'rsvelte compiler (napi)',
	swc_napi: 'swc (napi)'
} as const;

/** Absolute path to the bench harness's `node_modules` (where the alternative
 * impls and their native/wasm bindings install — the single dep tree both
 * runtimes consume; see package.json). Bindings sit at `<scope>/<pkg>/...`
 * with no version subdirectory (unlike the old Deno npm cache). */
function node_modules_dir(): string {
	return fileURLToPath(new URL('../node_modules', import.meta.url));
}

/** Get platform string for npm native binding packages (e.g., "linux-x64").
 * Translates the shared normalizer's Rust-style names to npm's. */
function get_npm_platform(): { os: string; arch: string } {
	const os = current_os() === 'windows' ? 'win32' : current_os();
	const arch =
		current_arch() === 'x86_64' ? 'x64' : current_arch() === 'aarch64' ? 'arm64' : current_arch();
	return { os, arch };
}

/** Try to stat a file and return its size, or null if it doesn't exist */
async function file_size(path: string): Promise<number | null> {
	try {
		return (await stat(path)).size;
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
		// `encoding: 'buffer'` so `stdout` is the raw gzip bytes (a string would
		// corrupt the binary and miscount). `maxBuffer` lifted well above the
		// default 1 MB — biome's wasm gzips to ~9 MB. Works under both runtimes
		// (Deno's node:child_process needs the bench's `--allow-env`/`--allow-run`).
		const { stdout } = await exec_file('gzip', ['-c', path], {
			encoding: 'buffer',
			maxBuffer: 256 * 1024 * 1024
		});
		return (stdout as unknown as Buffer).length;
	} catch {
		// Subprocess failed to spawn — gzip not on PATH (likely Windows without WSL).
		return null;
	}
}

/** What `collect_binary_sizes` measured, and what it reached for and didn't find. */
export interface CollectedBinarySizes {
	/** One row per artifact present on disk. */
	sizes: BinarySize[];
	/**
	 * Labels reached for but absent — the table's composition disclosure.
	 *
	 * Two different facts share this list, told apart by the label. A **tsv**
	 * variant (`tsv format (ffi)`, `tsv_parse_wasm`, …) is absent whenever its
	 * optional build task hasn't been run, which is routine on a machine that
	 * built only what it measures; the full `deno task bench` builds them all. A
	 * **third-party** label is absent although its impl initialized, which means
	 * the package shipped no artifact where this module looks — a stale path here,
	 * or an upstream layout change. The one third-party label with a benign
	 * reading is `oxc-parser (wasm)`: its binding lives in no manifest
	 * (`OXC_WASI_BINDING`, force-fetched by `deno task bench:install`), so a plain
	 * `npm install` leaves it absent with nothing else wrong.
	 */
	absent: string[];
}

/** A pre-gzip staged entry: known label/bytes/kind plus the path to compress. */
type StagedEntry = { entry: Omit<BinarySize, 'gzip_bytes'>; path: string };

/**
 * Staging state: the rows found, and the labels reached-for but absent.
 *
 * The absent list exists because this table is the one part of the report whose
 * COMPOSITION varies by machine — a row is emitted only if its file is on disk, so
 * an unbuilt artifact used to leave no trace at all, and a committed report's row
 * set silently described the producer's build state rather than the release. The
 * ratios read the same either way (`biome is 18.4x tsv`), which is what makes the
 * omission easy to miss.
 */
interface SizeStaging {
	found: StagedEntry[];
	absent: string[];
}

/** Add an entry for `path` if the file exists, else record the label as absent. */
async function push_size(
	out: SizeStaging,
	label: string,
	kind: BinaryKind,
	path: string
): Promise<void> {
	const bytes = await file_size(path);
	if (bytes !== null) out.found.push({ entry: { label, bytes, kind }, path });
	else out.absent.push(label);
}

/** Resolve the first existing file (by extension) under any of the candidate dirs. */
async function resolve_first(
	dirs: string[],
	ext: string
): Promise<{ path: string; bytes: number } | null> {
	for (const dir of dirs) {
		try {
			for (const e of await readdir(dir, { withFileTypes: true })) {
				if (e.isFile() && e.name.endsWith(ext)) {
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

/** Stage an entry resolved by scanning `dirs` for the first file ending in `ext`. */
async function push_resolved(
	out: SizeStaging,
	label: string,
	kind: BinaryKind,
	dirs: string[],
	ext: string
): Promise<void> {
	const found = await resolve_first(dirs, ext);
	if (found !== null) {
		out.found.push({ entry: { label, bytes: found.bytes, kind }, path: found.path });
	} else {
		out.absent.push(label);
	}
}

/**
 * Candidate cache dirs for a NAPI native binding, newest layout first. npm
 * ships per-libc variants (`-gnu`, `-musl`) plus a libc-agnostic fallback.
 */
function napi_binding_dirs(
	node_modules: string,
	scope_pkg: string,
	os: string,
	arch: string
): string[] {
	return [
		`${node_modules}/${scope_pkg}-${os}-${arch}-gnu`,
		`${node_modules}/${scope_pkg}-${os}-${arch}-musl`,
		`${node_modules}/${scope_pkg}-${os}-${arch}`
	];
}

/**
 * Collect binary sizes for all implementations.
 *
 * Uses known paths for tsv binaries (relative to project root)
 * and node_modules paths for npm packages. Computes gzipped size
 * alongside raw size; gzip is shelled out and parallelized across all
 * collected entries, so adding it costs roughly the slowest single
 * compression (biome's 35 MB dominates).
 *
 * Takes the initialized REGISTRY rather than a parallel list of `has_*` booleans.
 * That list was a second place to remember an impl, and the caller silently fell
 * behind it — three rows shipped with flags nothing ever passed, so a tool whose
 * init failed would still have been sized here (its package is installed either
 * way) and printed a size row with no benchmark row beside it. Reading `impls`
 * directly means an impl exists in exactly one place.
 */
export async function collect_binary_sizes(
	impls: ImplementationSet
): Promise<CollectedBinarySizes> {
	const project_root = fileURLToPath(new URL('../../..', import.meta.url));
	const node_modules = node_modules_dir();

	// Stage 1: collect (label, kind, path) for everything that exists, and the
	// labels reached-for but absent (see `SizeStaging`).
	const staged: SizeStaging = { found: [], absent: [] };

	// tsv native (FFI shared library) — `native`/`wasm` are required impls, so these
	// blocks are unconditional; an absent BUILD still shows up as `absent` below.
	const ffi_lib = native_library_filename('tsv_ffi');
	{
		await push_size(staged, LABELS.tsv_ffi, 'native', `${project_root}/target/release/${ffi_lib}`);
		// tsv format-only native — the native mirror of @fuzdev/tsv_format_wasm:
		// dropping the convert/JSON layer (and the parse exports) leaves a
		// scope-matched comparison against oxfmt (napi), which is format-only
		// too. Built into a separate target dir (deno task build:ffi:format) so it
		// doesn't clobber the full libtsv_ffi the perf rows load; omitted from the
		// table when that build hasn't been run.
		await push_size(
			staged,
			LABELS.tsv_format_ffi,
			'native',
			`${project_root}/target/ffi-format/release/${ffi_lib}`
		);
		// tsv parse-only native — the native mirror of @fuzdev/tsv_parse_wasm:
		// keeps the parse exports + the convert/JSON layer and drops the printers,
		// so it's scope-matched to oxc-parser (napi), which also materializes a
		// JSON AST. Separate target dir (deno task build:ffi:parse); omitted when
		// unbuilt.
		await push_size(
			staged,
			LABELS.tsv_parse_ffi,
			'native',
			`${project_root}/target/ffi-parse/release/${ffi_lib}`
		);
	}

	// tsv N-API addon — the Node/Bun native path (the sibling of the FFI library
	// Deno loads). Same engine, different binding boundary; sized from the built
	// cdylib (the shipped `.node` is a byte-identical copy). Existence-gated rather
	// than registry-gated: `impls.napi` is undefined under Deno, which loads the FFI
	// library instead, but the addon's size is worth reporting from either runtime.
	await push_size(
		staged,
		LABELS.tsv_napi,
		'native',
		`${project_root}/target/napi/${native_library_filename('tsv_napi')}`
	);

	// tsv WASM — three builds from one crate via the `format`/`parse` features:
	// pkg/format/deno (format-only, @fuzdev/tsv_format_wasm), pkg/parse/deno
	// (parse-only, @fuzdev/tsv_parse_wasm), and pkg/all/deno (both,
	// @fuzdev/tsv_wasm — the bundle the bench executes).
	{
		await push_size(
			staged,
			LABELS.tsv_format_wasm,
			'wasm',
			`${project_root}/crates/tsv_wasm/pkg/format/deno/tsv_wasm_bg.wasm`
		);
		await push_size(
			staged,
			LABELS.tsv_parse_wasm,
			'wasm',
			`${project_root}/crates/tsv_wasm/pkg/parse/deno/tsv_wasm_bg.wasm`
		);
		await push_size(
			staged,
			LABELS.tsv_wasm,
			'wasm',
			`${project_root}/crates/tsv_wasm/pkg/all/deno/tsv_wasm_bg.wasm`
		);
	}

	// biome WASM
	if (impls.biome) {
		await push_resolved(
			staged,
			LABELS.biome_wasm,
			'wasm',
			[`${node_modules}/@biomejs/wasm-bundler`],
			'.wasm'
		);
	}

	// dprint WASM (the `dprint-plugin-typescript` plugin — TS/JS only, so this is
	// scope-matched against the format-only builds, not the full tsv bundle)
	if (impls.dprint) {
		await push_resolved(
			staged,
			LABELS.dprint_wasm,
			'wasm',
			[`${node_modules}/@dprint/typescript`],
			'.wasm'
		);
	}

	// oxc-parser + oxfmt
	if (impls.oxc) {
		const { os: npm_os, arch: npm_arch } = get_npm_platform();

		await push_resolved(
			staged,
			LABELS.oxc_parser_napi,
			'native',
			napi_binding_dirs(node_modules, '@oxc-parser/binding', npm_os, npm_arch),
			'.node'
		);

		// oxfmt native binding (0.50.0+: @oxfmt/binding-{platform}; pre-0.49: @oxfmt/{platform}).
		await push_resolved(
			staged,
			LABELS.oxfmt_napi,
			'native',
			napi_binding_dirs(node_modules, '@oxfmt/binding', npm_os, npm_arch),
			'.node'
		);

		// oxc-parser WASM binding — named from the shared constant, so this lookup
		// tracks whatever `install_deps.ts` fetched and `check_node_modules.ts` graded.
		await push_resolved(
			staged,
			LABELS.oxc_parser_wasm,
			'wasm',
			[`${node_modules}/${OXC_WASI_BINDING}`],
			'.wasm'
		);
	}

	// yuku-parser — parse-only in both bindings, and yuku ships no formatter, so the
	// row a reader should pair each against is the parse-only tsv build (`tsv parse
	// (ffi)` / `tsv_parse_wasm`); against a bundle carrying the printers it would
	// size a scope difference and read as an engine one. The emitted `vs tsv` ratio
	// does NOT make that pairing — every row anchors on the full build (see
	// `build_display_entries`), so the parse-only rows are listed for the reader to
	// pair up, same as for `oxc-parser` and `dprint`. The wasm package is a plain dep
	// on every host (no `cpu: wasm32` metadata), so unlike oxc's wasi binding it
	// needs no force-fetch to be present.
	// Either binding present means the engine ran — one pair of size rows covers both.
	if (impls.yuku || impls.yuku_wasm) {
		const { os: npm_os, arch: npm_arch } = get_npm_platform();

		await push_resolved(
			staged,
			LABELS.yuku_parser_napi,
			'native',
			napi_binding_dirs(node_modules, '@yuku-parser/binding', npm_os, npm_arch),
			'.node'
		);

		await push_resolved(
			staged,
			LABELS.yuku_parser_wasm,
			'wasm',
			[`${node_modules}/@yuku-parser/wasm`],
			'.wasm'
		);
	}

	// rsvelte-fmt — a standalone EXECUTABLE, not a binding, so it's the one
	// native row that isn't scope-matched to a tsv artifact: it carries a CLI
	// (arg parsing, file discovery, config) plus the whole oxc formatter for
	// JS/TS/CSS alongside its Svelte engine, where `tsv (ffi)` is a bare library.
	// Listed anyway because omitting the only other Rust Svelte formatter from a
	// size table that lists every other alternative would read as an oversight;
	// read it as "what that tool ships", not as an engine-size comparison.
	// The platform-binary lookup is the only path here that can come up empty
	// BEFORE a file test, so it records the absence itself — every other reach is
	// a path handed to `push_size`/`push_resolved`, which record their own.
	if (impls.rsvelte) {
		const rsvelte_bin = rsvelte_binary_path();
		if (rsvelte_bin !== null) {
			await push_size(staged, LABELS.rsvelte_fmt_native, 'native', rsvelte_bin);
		} else {
			staged.absent.push(LABELS.rsvelte_fmt_native);
		}
	}

	// malva — dprint's CSS plugin wasm. Scope-matched to nothing tsv ships exactly
	// (tsv has no CSS-only build), so pair it against `tsv_format_wasm` in the
	// knowledge that malva formats one language where that build formats three.
	if (impls.malva) {
		await push_resolved(
			staged,
			LABELS.malva_wasm,
			'wasm',
			[`${node_modules}/dprint-plugin-malva`],
			'.wasm'
		);
	}

	// rsvelte's N-API addon — the artifact behind the `rsvelte-parse` rows, and like
	// `rsvelte-fmt (binary)` NOT scope-matched to a tsv build: it carries the whole
	// compiler plus `svelte2tsx`, HMR diffing and a resolver, where the rows measure
	// only its parser. Read it as "what that addon ships".
	if (impls.rsvelte_parse) {
		const { os: npm_os, arch: npm_arch } = get_npm_platform();
		await push_resolved(
			staged,
			LABELS.rsvelte_parse_napi,
			'native',
			napi_binding_dirs(node_modules, '@rsvelte/vite-plugin-svelte-native', npm_os, npm_arch),
			'.node'
		);
	}

	// swc — the same disclosure, more so: this `.node` is an entire compiler
	// (transforms, minifier, bundler entry points) where the row measures `parseSync`
	// alone, so it is the least scope-matched native entry in the table. Listed
	// because a table that sizes every other alternative would read as hiding it.
	if (impls.swc) {
		const { os: npm_os, arch: npm_arch } = get_npm_platform();
		await push_resolved(
			staged,
			LABELS.swc_napi,
			'native',
			napi_binding_dirs(node_modules, '@swc/core', npm_os, npm_arch),
			'.node'
		);
	}

	// Stage 2: gzip every collected file in parallel.
	const gzipped = await Promise.all(staged.found.map((s) => gzip_size(s.path)));

	return {
		sizes: staged.found.map(({ entry }, i) => ({ ...entry, gzip_bytes: gzipped[i] })),
		absent: staged.absent
	};
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
	// Native "vs tsv" anchor: the binding THIS runtime actually benchmarks — FFI
	// under Deno, N-API under Node/Bun — so the native ratios compare against the
	// row that produced the run's native timing, and a Node-only build (FFI lib
	// absent) still gets a populated anchor. Falls back to the FFI lib when the
	// runtime's own native binding isn't on disk.
	const native_anchor_label = current_runtime() === 'deno' ? LABELS.tsv_ffi : LABELS.tsv_napi;
	const tsv_native =
		sizes.find((s) => s.label === native_anchor_label) ??
		sizes.find((s) => s.label === LABELS.tsv_ffi);
	// "vs tsv" wasm anchor: the flagship full build (`tsv_wasm`), the artifact the
	// bench executes — identical `.wasm` across runtimes (only the JS glue differs).
	const tsv_wasm = sizes.find((s) => s.label === LABELS.tsv_wasm);

	const wasm_sizes = sizes.filter((s) => s.kind === 'wasm');
	const native_sizes = sizes.filter((s) => s.kind === 'native');

	// Build combined oxc-parser+oxfmt entry if both exist. Combined gzip is
	// the sum of the parts' gzipped sizes; that overstates wire size slightly
	// (two streams don't share a dictionary) but matches how npm ships them
	// — each binding is its own tarball.
	const oxc_parser = native_sizes.find((s) => s.label === LABELS.oxc_parser_napi);
	const oxfmt_entry = native_sizes.find((s) => s.label === LABELS.oxfmt_napi);
	const combined_oxc: BinarySize | null =
		oxc_parser && oxfmt_entry
			? {
					label: LABELS.oxc_combined_napi,
					bytes: oxc_parser.bytes + oxfmt_entry.bytes,
					gzip_bytes:
						oxc_parser.gzip_bytes !== null && oxfmt_entry.gzip_bytes !== null
							? oxc_parser.gzip_bytes + oxfmt_entry.gzip_bytes
							: null,
					kind: 'native'
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
			gzip_ratio: gzip_ratio_to(entry, reference)
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
		const ratio_str =
			ratio !== null
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
						format_ratio(gzip_ratio)
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
			'_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._'
		);
	}

	return lines.join('\n');
}

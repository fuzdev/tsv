/**
 * Artifact freshness guard for the rebuild-skipping bench/corpus/smoke tasks.
 *
 * `deno task bench` and `deno task corpus:compare:format` build the Rust + WASM
 * artifacts before running, so what they measure is fresh by construction. The
 * `:run` variants (`bench:deno:run` / `bench:node:run`, `corpus:compare:format:run`)
 * deliberately SKIP that build — that's the path for iterating on the measurement/reporting harness
 * (this directory's `.ts`) without paying the cargo + wasm-pack cost on every
 * tweak. `deno task smoke` likewise skips the build (it's the fast pre-bench
 * sanity check) and relies on this guard rather than rebuilding.
 *
 * The hazard the split creates: edit a crate's source, run a `:run` task, and
 * you silently measure the *previously* built binary. That is exactly the trap
 * that once made a CSS corpus run report 146/183 against a three-day-old `.so`
 * when current source handled 155/183.
 *
 * This module stats the crate sources that feed each executed artifact against
 * that artifact's own mtime and aborts the run when any source is newer (or the
 * artifact is missing). It only guards artifacts that are actually *executed*
 * during measurement, and WHICH those are is a runtime fact this module owns
 * (`native_artifact_check` / `check_executed_artifacts`): the native binding the
 * runtime loads — C-FFI under Deno, the N-API addon under Node/Bun — plus that
 * runtime's `pkg/all/{deno,nodejs}` WASM bundle, the full build supplying both the
 * parse and format functions the bench runs. The corpus tools execute no WASM, so
 * they guard the native library alone.
 *
 * Size-only artifacts — the subset `pkg/{format,parse}/<target>` bundles and the
 * `target/ffi-{format,parse}` builds — aren't guarded: nothing runs them, and
 * `binary_sizes.ts` both degrades gracefully when they're absent and now records
 * the absence (`binary_sizes_absent`), so a report from a partially-built tree
 * says so rather than just carrying a shorter table.
 *
 * Escape hatch: set `BENCH_STALE_OK=1` to run anyway. A missing artifact is
 * always fatal (you can't measure what isn't there); `BENCH_STALE_OK=1`
 * downgrades a *stale* (present-but-older) artifact to a one-line warning so a
 * deliberate stale run stays possible and stays visible in the output.
 *
 * Running this on the build-first tasks is harmless: a freshly built artifact
 * is newer than its sources, so the check passes silently.
 */

import { readdir, stat } from 'node:fs/promises';
import { env, exit } from 'node:process';
import { fileURLToPath } from 'node:url';
import { get_library_path } from './ffi.ts';
import { get_napi_library_path } from './napi.ts';
import { current_runtime } from './runtime.ts';

/**
 * Crates whose source compiles into EVERY measured tsv artifact (the shared
 * core): the language crates plus `tsv_arena` (all three bindings' per-thread
 * reuse). Applied as the freshness floor for every check.
 *
 * `tsv_ignore` + `tsv_discover` are deliberately NOT here — they feed only the
 * WASM bundle (the `IgnoreStack` export), not `tsv_ffi` / `tsv_napi`, so they
 * live in `WASM_CRATES`. Sharing them would false-stale the native checks: a
 * `tsv_discover` edit never rebuilds the FFI (it's not in its dependency
 * graph), so the guard could never clear on a rebuild.
 *
 * Exported (with `WASM_CRATES` + `newest_source_mtime`) for
 * `scripts/run_if_stale.ts`, the build-side sibling — the two sides must agree
 * on what "the sources" are. Deliberately excludes the dev-tooling crates
 * (`tsv_debug`, `tsv_cli`): they don't feed the measured artifacts, and
 * including them would force wasm rebuilds on every fixture-workflow edit.
 */
export const CORE_CRATES = ['tsv_lang', 'tsv_arena', 'tsv_html', 'tsv_ts', 'tsv_css', 'tsv_svelte'];

/**
 * Crates that feed the WASM bundle beyond `CORE_CRATES`: the binding crate
 * itself plus `tsv_ignore` + `tsv_discover` (the `IgnoreStack` export, which
 * only the WASM artifact links among the measured bindings — `tsv_ffi` /
 * `tsv_napi` link neither). Used as the WASM check's `binding_crates` AND by
 * `scripts/run_if_stale.ts`, imported by both so the run-side guard and the
 * build-side skip can't drift on what feeds the bundle.
 */
export const WASM_CRATES = ['tsv_wasm', 'tsv_ignore', 'tsv_discover'];

/** Absolute path to the workspace `crates/` directory. */
const CRATES_DIR = fileURLToPath(new URL('../../../crates', import.meta.url));

/** Absolute path to the workspace `Cargo.lock` (dependency bumps must also trip staleness). */
const CARGO_LOCK = fileURLToPath(new URL('../../../Cargo.lock', import.meta.url));

export interface ArtifactCheck {
	/** Human-readable label used in messages, e.g. `FFI (release)`. */
	label: string;
	/** Absolute path to the built artifact file. */
	path: string;
	/**
	 * Binding crate(s) feeding this artifact, beyond `CORE_CRATES` — e.g.
	 * `['tsv_ffi']` for the native library, `['tsv_wasm']` for a WASM bundle.
	 */
	binding_crates: string[];
	/** Command that rebuilds this artifact, surfaced in the error message. */
	rebuild: string;
}

export interface SourceMtime {
	/** Newest mtime in milliseconds (0 if no sources were found). */
	ms: number;
	/** `crates/`-relative path of the newest source, for the message. */
	path: string;
}

interface StaleArtifact {
	label: string;
	path: string;
	reason: 'missing' | 'stale';
	rebuild: string;
	/** Newest source path + mtimes, present only for `reason: 'stale'`. */
	source_path?: string;
	artifact_ms?: number;
	source_ms?: number;
}

/** Format an mtime as a compact local `MM-DD HH:MM` stamp for messages. */
function fmt_mtime(ms: number): string {
	const d = new Date(ms);
	const p = (n: number): string => String(n).padStart(2, '0');
	return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

const _mtime_cache = new Map<string, SourceMtime>();

/** Newest mtime across `*.rs` files and `Cargo.toml` under the given crates. Memoized per crate set. */
export async function newest_source_mtime(crates: string[]): Promise<SourceMtime> {
	const key = crates.join(',');
	const cached = _mtime_cache.get(key);
	if (cached) return cached;

	let newest: SourceMtime = { ms: 0, path: '' };
	const consider = (ms: number, rel: string): void => {
		if (ms > newest.ms) newest = { ms, path: rel };
	};

	for (const crate of crates) {
		const root = `${CRATES_DIR}/${crate}`;
		try {
			const st = await stat(`${root}/Cargo.toml`);
			consider(st.mtimeMs, `${crate}/Cargo.toml`);
		} catch {
			// crate may not have a Cargo.toml at this path — ignore
		}
		try {
			const src = `${root}/src`;
			for (const relative of await readdir(src, { recursive: true })) {
				if (!relative.endsWith('.rs')) continue;
				const full = `${src}/${relative}`;
				const st = await stat(full);
				if (!st.isFile()) continue;
				consider(st.mtimeMs, full.slice(CRATES_DIR.length + 1));
			}
		} catch {
			// crate may not have a src/ directory — ignore
		}
	}

	_mtime_cache.set(key, newest);
	return newest;
}

/**
 * Abort the current `:run` task if any executed artifact is missing or older
 * than the crate sources that feed it. See the module doc for the rationale and
 * the `BENCH_STALE_OK=1` escape hatch. Exits the process with code 1 on a fatal
 * staleness; returns normally when everything is fresh (or only warns).
 */
export async function check_artifact_freshness(checks: readonly ArtifactCheck[]): Promise<void> {
	const stale_ok = env.BENCH_STALE_OK === '1';
	let core = await newest_source_mtime(CORE_CRATES);
	try {
		const lock = await stat(CARGO_LOCK);
		if (lock.mtimeMs > core.ms) core = { ms: lock.mtimeMs, path: 'Cargo.lock' };
	} catch {
		// no lockfile (fresh clone pre-build) — the missing-artifact check governs
	}

	const stale: StaleArtifact[] = [];
	for (const check of checks) {
		const binding = await newest_source_mtime(check.binding_crates);
		const source = binding.ms > core.ms ? binding : core;

		let artifact_ms: number;
		try {
			const st = await stat(check.path);
			artifact_ms = st.mtimeMs;
		} catch {
			stale.push({
				label: check.label,
				path: check.path,
				reason: 'missing',
				rebuild: check.rebuild
			});
			continue;
		}

		// Strict `<` so an artifact built in the same second as its source passes.
		if (artifact_ms < source.ms) {
			stale.push({
				label: check.label,
				path: check.path,
				reason: 'stale',
				rebuild: check.rebuild,
				source_path: source.path,
				artifact_ms,
				source_ms: source.ms
			});
		}
	}

	if (stale.length === 0) return;

	const has_missing = stale.some((s) => s.reason === 'missing');
	const fatal = has_missing || !stale_ok;

	const lines: string[] = [];
	lines.push('');
	lines.push(
		fatal
			? '✗ Stale benchmark artifacts — refusing to measure outdated binaries.'
			: '⚠ Stale benchmark artifacts (BENCH_STALE_OK=1 — measuring anyway).'
	);
	for (const s of stale) {
		if (s.reason === 'missing') {
			lines.push(`  • ${s.label}: not built — ${s.path}`);
		} else {
			lines.push(
				`  • ${s.label}: built ${fmt_mtime(s.artifact_ms!)}, ` +
					`but ${s.source_path} changed ${fmt_mtime(s.source_ms!)}`
			);
		}
		lines.push(`      rebuild: ${s.rebuild}`);
	}
	if (fatal) {
		lines.push('');
		lines.push('  Rebuild everything first with a build-first task (`deno task bench` /');
		lines.push(
			'  `deno task corpus:compare:format`), or `deno task build:bench` then re-run `deno task smoke`,'
		);
		lines.push('  run the specific rebuild command(s) above, or set BENCH_STALE_OK=1 to override');
		lines.push('  (the override applies to stale artifacts only — a missing one is always fatal).');
	}
	lines.push('');

	console.error(lines.join('\n'));
	if (fatal) exit(1);
}

/** Path to the executed WASM bundle's compiled `.wasm` for the given variant —
 * the runtime's own wasm-pack target (Deno → `deno`, Node/Bun → `nodejs`). */
export function wasm_artifact_path(variant: 'format' | 'parse' | 'all'): string {
	const target = current_runtime() === 'deno' ? 'deno' : 'nodejs';
	return fileURLToPath(
		new URL(`../../../crates/tsv_wasm/pkg/${variant}/${target}/tsv_wasm_bg.wasm`, import.meta.url)
	);
}

/**
 * The check for the native binding this runtime EXECUTES: the C-FFI library
 * under Deno (`Deno.dlopen`), the N-API addon under Node/Bun (`process.dlopen`).
 * Same engine, different binding boundary — which one is live is a runtime fact,
 * so it is answered here rather than at each entry point.
 *
 * The rebuild hint follows `TSV_FFI_PROFILE`: a `corpus`-profile library is
 * rebuilt by a different task than the release one, and pointing a reader at
 * `build:ffi` when their stale artifact is the corpus build is a remedy that
 * rebuilds the wrong file. That correction lived in `compare_cli.ts` alone while
 * the two copies of this block went without it.
 */
export function native_artifact_check(): ArtifactCheck {
	if (current_runtime() !== 'deno') {
		return {
			label: 'N-API',
			path: get_napi_library_path(),
			binding_crates: ['tsv_napi'],
			rebuild: 'deno task build:napi'
		};
	}
	const profile = env.TSV_FFI_PROFILE ?? 'release';
	return {
		label: `FFI (${profile})`,
		path: get_library_path(),
		binding_crates: ['tsv_ffi'],
		rebuild: profile === 'corpus' ? 'deno task build:ffi:corpus' : 'deno task build:ffi'
	};
}

/**
 * Guard every artifact a bench/smoke run executes — the runtime's native binding
 * plus its `all` WASM bundle.
 *
 * `bench.ts` and `smoke.ts` had this block verbatim, twice: the same
 * runtime-conditional native entry, the same WASM entry, the same target
 * derivation. That pairing IS this module's subject ("which artifacts does this
 * runtime execute"), so a third measured binding, or a moved output path, is one
 * edit here rather than a sweep of the entry points — the failure mode being a
 * run that guards one artifact and silently measures another.
 *
 * The corpus tools guard only the FFI library (they run no WASM), so they call
 * `native_artifact_check` and pass it themselves.
 */
export async function check_executed_artifacts(): Promise<void> {
	const target = current_runtime() === 'deno' ? 'deno' : 'nodejs';
	await check_artifact_freshness([
		native_artifact_check(),
		{
			label: `WASM (all/${target})`,
			path: wasm_artifact_path('all'),
			binding_crates: WASM_CRATES,
			rebuild: `deno task build:wasm:all:${target}`
		}
	]);
}

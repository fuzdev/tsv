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
 * Size-only artifacts — every tsv build the report SIZES but this runtime does not
 * execute (`lib/tsv_artifacts.ts`: the other binding, the subset bundles, the
 * `target/ffi-{format,parse}` builds) — are graded too, but never fatally
 * (`warn_stale_reported_artifacts`): nothing measures them, so a stale one can't
 * corrupt a timing, and a `:run` exists precisely to skip their rebuild. What it CAN
 * do is publish the other binding's size at an older commit inside a report stamped
 * with this one, which is the warning. An absent one is `binary_sizes.ts`'s to
 * record (`binary_sizes_absent`), so only staleness is named here.
 *
 * Escape hatch: set `BENCH_STALE_OK=1` to run anyway. A missing artifact is
 * always fatal (you can't measure what isn't there); `BENCH_STALE_OK=1`
 * downgrades a *stale* (present-but-older) artifact to a one-line warning so a
 * deliberate stale run stays possible and stays visible in the output.
 *
 * Running this on the build-first tasks is harmless: a freshly built artifact
 * is newer than its sources, so the check passes silently. For a cargo-built
 * artifact that holds only because the build tasks STAMP it
 * (`scripts/cargo_build.ts`, `deno task build:stamped`): cargo keys `Cargo.lock`
 * and a manifest by content, so one rewritten to something already built makes the
 * build a no-op that re-links the cached artifact under its original date — older
 * than the file this guard compares it to. The stamp dates the artifact at cargo's
 * last confirmation instead, which is also what makes every `rebuild:` hint below
 * able to clear the refusal it is printed under. A bare `cargo build` does not
 * stamp, so after one the hint is still the remedy.
 */

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { readdir, stat } from 'node:fs/promises';
import { resolve } from 'node:path';
import { env, exit } from 'node:process';
import { fileURLToPath } from 'node:url';
import { get_library_path } from './ffi.ts';
import { get_napi_library_path } from './napi.ts';
import { current_runtime, wasm_target } from './runtime.ts';
import { CORE_CRATES, TSV_ARTIFACTS, wasm_bundle_path } from './tsv_artifacts.ts';

/** Absolute path to the workspace `crates/` directory. */
const CRATES_DIR = fileURLToPath(new URL('../../../crates', import.meta.url));

/** Absolute path to the workspace root, with its trailing separator. */
const ROOT = fileURLToPath(new URL('../../../', import.meta.url));

/**
 * The workspace-level files every cargo build reads beside its crates' own sources:
 * the root manifest (every `[profile.*]`, the workspace dependencies, the version) and
 * the lockfile (what those dependencies resolved to). Stated once, for every mtime
 * guard — a profile edit that staled one guard's artifact and not another's is two
 * guards disagreeing about what a build is made of.
 */
const WORKSPACE_BUILD_FILES = ['Cargo.toml', 'Cargo.lock'];

export interface ArtifactCheck {
	/** Human-readable label used in messages, e.g. `FFI (release)`. */
	label: string;
	/** Absolute path to the built artifact file. */
	path: string;
	/**
	 * Binding crate(s) feeding this artifact, beyond `CORE_CRATES` — e.g.
	 * `['tsv_ffi']` for the native library, `['tsv_wasm']` for a WASM bundle.
	 */
	binding_crates: readonly string[];
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

/** Format an mtime as a compact local `MM-DD HH:MM` stamp for messages.
 * Exported for `scripts/check_staged_freshness.ts`, whose messages match. */
export function fmt_mtime(ms: number): string {
	const d = new Date(ms);
	const p = (n: number): string => String(n).padStart(2, '0');
	return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

const _mtime_cache = new Map<string, SourceMtime>();

/**
 * Newest mtime across every file under `src/`, the crate's `build.rs`, and its
 * `Cargo.toml`, for the given crates. Memoized per crate set. Every file under `src/`,
 * not `*.rs` alone: a build script's inputs sit there too (`tsv_html`'s
 * `src/entities.json`, which `build.rs` compiles into the entity table), and a walk
 * that saw only Rust let a codegen change stale nothing on any artifact.
 */
export async function newest_source_mtime(crates: readonly string[]): Promise<SourceMtime> {
	const key = crates.join(',');
	const cached = _mtime_cache.get(key);
	if (cached) return cached;

	let newest: SourceMtime = { ms: 0, path: '' };
	const consider = (ms: number, rel: string): void => {
		if (ms > newest.ms) newest = { ms, path: rel };
	};

	for (const crate of crates) {
		const root = `${CRATES_DIR}/${crate}`;
		for (const manifest of ['Cargo.toml', 'build.rs']) {
			try {
				const st = await stat(`${root}/${manifest}`);
				consider(st.mtimeMs, `${crate}/${manifest}`);
			} catch {
				// no such file for this crate — ignore
			}
		}
		try {
			const src = `${root}/src`;
			for (const relative of await readdir(src, { recursive: true })) {
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
 * Newest mtime across `WORKSPACE_BUILD_FILES`, labeled by file name (`ms: 0` when
 * none exists). The floor every cargo-built artifact is dated against, whatever
 * crates feed it.
 */
export async function newest_workspace_file_mtime(): Promise<SourceMtime> {
	let newest: SourceMtime = { ms: 0, path: '' };
	for (const file of WORKSPACE_BUILD_FILES) {
		try {
			const st = await stat(`${ROOT}${file}`);
			if (st.mtimeMs > newest.ms) newest = { ms: st.mtimeMs, path: file };
		} catch {
			// no lockfile (fresh clone pre-build) — the crate sources govern
		}
	}
	return newest;
}

/** Every check whose artifact is missing or older than the sources feeding it. */
async function find_stale(checks: readonly ArtifactCheck[]): Promise<StaleArtifact[]> {
	let core = await newest_source_mtime(CORE_CRATES);
	const workspace = await newest_workspace_file_mtime();
	if (workspace.ms > core.ms) core = workspace;

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
	return stale;
}

/**
 * Abort the current `:run` task if any executed artifact is missing or older
 * than the crate sources that feed it. See the module doc for the rationale and
 * the `BENCH_STALE_OK=1` escape hatch. Exits the process with code 1 on a fatal
 * staleness; returns normally when everything is fresh (or only warns).
 */
export async function check_artifact_freshness(checks: readonly ArtifactCheck[]): Promise<void> {
	const stale_ok = env.BENCH_STALE_OK === '1';
	const stale = await find_stale(checks);
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
	await check_artifact_freshness(executed_artifact_checks());
}

/**
 * What one executed artifact WAS when a run's pre-flight graded it — its bytes'
 * digest — so a later process of the run can prove it loads the same file.
 */
export interface ArtifactIdentity {
	label: string;
	path: string;
	/** sha1 of the file's bytes. */
	sha1: string;
}

/**
 * The identity of every artifact this runtime executes (`executed_artifact_checks`),
 * read from disk now. By content, not mtime: the build tasks re-stamp an artifact's
 * date on a cargo no-op (`deno task build:stamped`), so a `build:bench` that rebuilt
 * nothing would otherwise read as a different binary.
 *
 * @throws if an artifact is missing — the freshness guard has already required them
 */
export function executed_artifact_identities(): ArtifactIdentity[] {
	return executed_artifact_checks().map(({ label, path }) => ({
		label,
		path,
		sha1: createHash('sha1').update(readFileSync(path)).digest('hex')
	}));
}

/**
 * Throw unless every artifact in `expected` still has the bytes it had then.
 *
 * A bench run is hours of fresh processes, each loading the artifacts from disk
 * again, so a build in the same checkout partway through (a `cargo build`, a
 * `build:bench`) would have the later passes time a binary the pre-flight never
 * graded — its accept sets, its byte parity, the size rows — with nothing to say so.
 * Each timed row's process asks this before it loads anything.
 *
 * @throws if any artifact changed or is gone, naming each
 */
export function assert_artifacts_unchanged(expected: readonly ArtifactIdentity[]): void {
	const changed: string[] = [];
	for (const artifact of expected) {
		let sha1: string;
		try {
			sha1 = createHash('sha1').update(readFileSync(artifact.path)).digest('hex');
		} catch {
			changed.push(`${artifact.label} (${artifact.path}) is gone`);
			continue;
		}
		if (sha1 !== artifact.sha1) changed.push(`${artifact.label} (${artifact.path}) was rebuilt`);
	}
	if (changed.length > 0) {
		throw new Error(
			`an artifact changed since this run's pre-flight graded it — ${changed.join('; ')}. ` +
				`A build ran in this checkout mid-run; re-run the bench once it is done.`
		);
	}
}

/** The checks `check_executed_artifacts` runs — the runtime's native binding + its `all` bundle. */
function executed_artifact_checks(): ArtifactCheck[] {
	const target = wasm_target();
	return [
		native_artifact_check(),
		{
			label: `WASM (all/${target})`,
			// This runtime's own wasm-pack target (Deno → `deno`, Node/Bun → `nodejs`),
			// which is why it is not `TSV_ARTIFACTS.tsv_wasm.path`: that row reports the
			// `deno` bundle from every runtime, and under Node/Bun the executed one is a
			// different file (correctly graded as size-only there).
			path: wasm_bundle_path('all', target),
			binding_crates: TSV_ARTIFACTS.tsv_wasm.binding_crates,
			rebuild: `deno task build:wasm:all:${target}`
		}
	];
}

/**
 * Warn about every tsv artifact the report SIZES but this runtime does not execute
 * (`TSV_ARTIFACTS` minus `executed_artifact_checks`) that is older than its sources.
 * Never fatal, and silent on a missing one (that is `binary_sizes_absent`'s claim):
 * nothing measures these, so the only harm is the size table publishing the other
 * binding's build at an older commit under this run's `git_commit` — which the
 * build-first tasks make unreachable (`build:bench` builds the whole set) and a
 * `:run` after a crate edit does not. `BENCH_STALE_OK` is irrelevant here: the
 * warning is the whole response.
 */
export async function warn_stale_reported_artifacts(): Promise<void> {
	// Both sides now come from `tsv_artifacts.ts`'s builders, so a path that DIFFERS
	// here differs for a reason rather than by spelling — the two axes a loader owns.
	// A `TSV_FFI_PROFILE=corpus` run executes `target/corpus` while the size table
	// reports `target/release`, and Node/Bun execute the `nodejs` bundle while the
	// table reports the `deno` one; in both cases the reported build really is
	// unmeasured here, which is exactly what this warning is about. `resolve` stays
	// as normalization, not as the thing holding the comparison together.
	const executed = new Set(executed_artifact_checks().map((c) => resolve(c.path)));
	const checks: ArtifactCheck[] = Object.values(TSV_ARTIFACTS)
		.filter((a) => !executed.has(resolve(a.path)))
		.map((a) => ({
			label: a.label,
			path: a.path,
			binding_crates: a.binding_crates,
			rebuild: a.rebuild
		}));
	const stale = (await find_stale(checks)).filter((s) => s.reason === 'stale');
	if (stale.length === 0) return;
	const lines = [
		'',
		'⚠ Stale size-only artifacts — the binary-size table will report an older build:'
	];
	for (const s of stale) {
		lines.push(
			`  • ${s.label}: built ${fmt_mtime(s.artifact_ms!)}, but ${s.source_path} changed ` +
				`${fmt_mtime(s.source_ms!)}\n      rebuild: ${s.rebuild}`
		);
	}
	lines.push(
		'  (not measured, so not fatal — `deno task build:bench` refreshes the whole set)',
		''
	);
	console.error(lines.join('\n'));
}

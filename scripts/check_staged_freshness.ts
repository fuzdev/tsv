/**
 * Freshness guard for the STAGED npm-package artifacts the `:run` test tasks
 * consume — the staged-package sibling of
 * `benches/js/lib/check_artifact_freshness.ts` (the bench/corpus `:run` guard).
 *
 * `deno task test:npm[:parse|:all]` and `deno task test:napi:npm` build first,
 * so what they test is fresh by construction. Their `:run` variants
 * deliberately skip that build — the path for iterating on the test harness —
 * at the risk of silently testing a STALE staging: the incident this guard
 * exists for was `crates/tsv_napi/pkg` holding a native `tsv_cli` binary from
 * before a behavior fix, which `test:napi:npm:run` would have green-tested
 * without a word. `scripts/validate_artifacts.ts` is the third consumer, for
 * the same reason with no `:run` split at all — it never builds, so a bare
 * `deno task validate:artifacts` grades whatever `pkg/` happens to hold.
 *
 * Two postures, one grading. `assert_staged_fresh` ABORTS, which is right over
 * an artifact the caller's own task builds. `staged_staleness` grades one
 * artifact and hands the reason back, for a caller whose right answer is to
 * SKIP — a check reaching into a package it cannot rebuild, where failing
 * would only be telling the operator to run someone else's build (that is
 * `test_napi_npm.ts` over `@fuzdev/tsv-wasm`). Both read the same mtimes, so a
 * skip and an abort never disagree about what is stale.
 *
 * The check LISTS for a staged wasm package and the staged napi loader are stated here
 * once (`wasm_package_checks`, `napi_loader_checks`), so every reader of one staging — the
 * package suites, artifact validation, the napi suite's export-set parity, the engine
 * parity audit, `scripts/typecheck_packages.ts` and the Bun leg (`scripts/test_bun.ts`) — dates it by the same sources. A reader with an artifact
 * of its own (the napi platform package's CLI binary, the deno bundles, `engine_parity.ts`'s
 * native binary) adds that check beside the shared list; the platform package's addon has
 * one of its own, `napi_addon_check`, since two suites load it.
 *
 * Staleness here has two lags — the `target/` build behind the sources, and the
 * staged copy behind the build — and comparing the staged file's mtime directly
 * against the SOURCES catches both with one check.
 *
 * Each check names the crates and/or individual files that feed one staged
 * artifact; when crates are named, the workspace `Cargo.toml` + `Cargo.lock`
 * are considered too (dependency bumps rebuild the artifact). A missing staged
 * file is always fatal; `BENCH_STALE_OK=1` — the same escape hatch as the
 * bench guard, deliberately one knob for every mtime guard — downgrades a
 * stale (present-but-older) one to a warning. What no mtime check can see is a
 * toolchain change; after one, rebuild once via the build-first task.
 */

import { existsSync, readdirSync } from 'node:fs';
import { stat } from 'node:fs/promises';
import { relative } from 'node:path';
import { env, exit } from 'node:process';
import { fileURLToPath } from 'node:url';

import { fmt_mtime, newest_source_mtime } from '../benches/js/lib/check_artifact_freshness.ts';
import {
	CORE_CRATES,
	WASM_CRATES,
	wasm_bundle_dir,
	type WasmVariant
} from '../benches/js/lib/tsv_artifacts.ts';
import { ALL_FAMILIES, facade_sources } from './npm_facade.ts';

const ROOT = fileURLToPath(new URL('..', import.meta.url));

/** One staged artifact to check against the sources that feed it. */
export interface StagedCheck {
	/** Human-readable label used in messages. */
	label: string;
	/** Repo-root-relative path to the staged file. */
	staged: string;
	/** Crates under `crates/` whose Rust sources compile into it (may be empty). */
	crates: string[];
	/** Repo-root-relative individual source files (staging scripts, copied JS). */
	files: string[];
	/** Command that restages this artifact, surfaced in the error message. */
	rebuild: string;
}

/** Why one staged artifact is not fresh — `staged_staleness`'s verdict. */
export interface StagedStaleness {
	/** The staged file is absent, not merely old. Always fatal, never overridable. */
	missing: boolean;
	/** One line naming what is stale (or absent); carries no restage hint. */
	reason: string;
}

/**
 * Grade ONE staged artifact against the sources feeding it and return why it
 * is not fresh, without aborting — the seam for a caller that wants to SKIP
 * rather than fail, which is the right posture over a package the caller does
 * not itself build. `assert_staged_fresh` is this over a list, plus the abort.
 * The one throw is a bug in the CHECK, not a verdict about the artifact: a
 * `files` entry naming a path that does not exist.
 */
export async function staged_staleness(check: StagedCheck): Promise<StagedStaleness | undefined> {
	let staged_ms: number;
	try {
		staged_ms = (await stat(`${ROOT}${check.staged}`)).mtimeMs;
	} catch {
		return { missing: true, reason: `${check.label}: not staged — ${check.staged}` };
	}

	let newest = { ms: 0, path: '' };
	if (check.crates.length > 0) {
		newest = { ...(await newest_source_mtime(check.crates)) };
		for (const workspace_file of ['Cargo.toml', 'Cargo.lock']) {
			try {
				const st = await stat(`${ROOT}${workspace_file}`);
				if (st.mtimeMs > newest.ms) newest = { ms: st.mtimeMs, path: workspace_file };
			} catch {
				// no lockfile (fresh clone pre-build) — the crate sources govern
			}
		}
	}
	for (const file of check.files) {
		let st;
		try {
			st = await stat(`${ROOT}${file}`);
		} catch {
			// a named source that does not exist is a typo in the check, and a typo'd
			// path would otherwise degrade the check to no check at all
			throw new Error(`${check.label}: named source ${file} does not exist — fix the check`);
		}
		if (st.mtimeMs > newest.ms) newest = { ms: st.mtimeMs, path: file };
	}

	// Strict `<` so an artifact staged in the same second as its source passes.
	if (staged_ms < newest.ms) {
		return {
			missing: false,
			reason:
				`${check.label}: staged ${fmt_mtime(staged_ms)}, ` +
				`but ${newest.path} changed ${fmt_mtime(newest.ms)}`
		};
	}
	return undefined;
}

/**
 * Abort (exit 1) when any staged artifact is missing or older than a source
 * feeding it; see the module doc for the escape hatch. Returns normally when
 * everything is fresh (or staleness was downgraded to a warning).
 */
export async function assert_staged_fresh(checks: readonly StagedCheck[]): Promise<void> {
	const stale_ok = env.BENCH_STALE_OK === '1';
	const findings: string[] = [];
	let missing = false;

	for (const check of checks) {
		const staleness = await staged_staleness(check);
		if (!staleness) continue;
		if (staleness.missing) missing = true;
		findings.push(`  • ${staleness.reason}`);
		findings.push(`      restage: ${check.rebuild}`);
	}

	if (findings.length === 0) return;

	const fatal = missing || !stale_ok;
	console.error(
		[
			'',
			fatal
				? '✗ Stale staged package artifacts — refusing to grade outdated code.'
				: '⚠ Stale staged package artifacts (BENCH_STALE_OK=1 — grading anyway).',
			...findings,
			...(fatal
				? [
						'',
						'  Run the restage command(s) above (or the build-first task, where a `:run`',
						'  variant skipped it), or set BENCH_STALE_OK=1 to override (stale only —',
						'  a missing staged file is always fatal).'
					]
				: []),
			''
		].join('\n')
	);
	if (fatal) exit(1);
}

/** The staging scripts and metadata that write a wasm package's generated entries. */
const PATCHER_SOURCES = [
	'scripts/patch_npm_package.ts',
	'scripts/npm_facade.ts',
	'scripts/npm_metadata.ts'
];

/** A wasm variant's npm staging (`scripts/patch_npm_package.ts` writes it), repo-relative. */
export function wasm_package_dir(variant: WasmVariant): string {
	return relative(ROOT, wasm_bundle_dir(variant, 'npm'));
}

/**
 * The checks that date one staged wasm package — the one list every reader of a
 * `pkg/<variant>/npm` staging grades it by (the package suite, artifact validation, the
 * napi suite's export-set parity, the engine parity audit, the declaration check, the Bun leg). One check per staging step, keyed
 * on a file that step writes: the bundle (wasm-pack, whose features are a `deno.json` task
 * string), the generated entries (the patcher and its inputs), the copied facade, and for
 * the `all` variant the copied `cli.js`.
 */
export function wasm_package_checks(
	variant: WasmVariant,
	dir: string = wasm_package_dir(variant)
): Array<StagedCheck> {
	const rebuild = `deno task build:npm:${variant}`;
	return [
		{
			label: `staged WASM bundle (${variant})`,
			staged: `${dir}/tsv_wasm_bg.wasm`,
			crates: [...CORE_CRATES, ...WASM_CRATES],
			files: ['deno.json'],
			rebuild
		},
		{
			// the patcher derives the entries from wasm-pack's generated JS, so whatever feeds
			// that (the crates, the feature set in deno.json) feeds them too — a wasm-pack rerun
			// the patcher did not follow must stale them, and `cli.js`, written in the same pass
			label: `staged package entries (${variant})`,
			staged: `${dir}/index.js`,
			crates: [...CORE_CRATES, ...WASM_CRATES],
			files: [...PATCHER_SOURCES, 'deno.json'],
			rebuild
		},
		{
			// every facade file the variant ships — the AST types included — is copied in
			// the same staging step, so the first one's age speaks for the rest
			label: `staged facade (${variant})`,
			staged: `${dir}/api.js`,
			crates: [],
			files: [
				...facade_sources({ format: variant !== 'parse', parse: variant !== 'format' }),
				'scripts/npm_facade.ts'
			],
			rebuild
		},
		...(variant === 'all'
			? [
					{
						label: 'staged cli.js (all)',
						staged: `${dir}/cli.js`,
						crates: [],
						files: ['crates/tsv_wasm/npm/cli.js'],
						rebuild
					}
				]
			: [])
	];
}

/** Where `scripts/build_napi_packages.ts` stages the N-API packages, repo-relative. */
export const NAPI_PKG_ROOT = 'crates/tsv_napi/pkg';

/** The `@fuzdev/tsv` loader's staging. */
export const NAPI_LOADER_DIR = `${NAPI_PKG_ROOT}/napi`;

/**
 * The triple of the one platform package staged beside the loader — the BUILD script's
 * host detection, which a loader resolving its sibling by its own detection then agrees
 * with or fails. Exits naming what is staged when the loader is absent or the platform
 * packages are not exactly one.
 */
export function napi_staged_triple(): string {
	if (!existsSync(`${ROOT}${NAPI_LOADER_DIR}`)) {
		console.error(`${NAPI_LOADER_DIR} not staged. Run 'deno task build:napi:packages' first.`);
		exit(1);
	}
	const platform_dirs = readdirSync(`${ROOT}${NAPI_PKG_ROOT}`).filter((d) => d !== 'napi');
	if (platform_dirs.length !== 1) {
		console.error(
			`expected exactly one staged platform package under ${NAPI_PKG_ROOT}, got: ${platform_dirs.join(', ') || '(none)'}`
		);
		exit(1);
	}
	return platform_dirs[0]!;
}

/** The check that dates the staged N-API addon in the platform package for `triple`. */
export function napi_addon_check(triple: string): StagedCheck {
	return {
		label: 'staged N-API addon',
		staged: `${NAPI_PKG_ROOT}/${triple}/tsv_napi.node`,
		// the addon links the discovery crates too (its `format` feature pulls `tsv_ignore` +
		// `tsv_discover` for the `IgnoreStack` export), so an edit there must stale it
		crates: [...CORE_CRATES, 'tsv_napi', 'tsv_ignore', 'tsv_discover'],
		files: ['scripts/build_napi_packages.ts'],
		rebuild: 'deno task build:napi:packages'
	};
}

/**
 * The checks that date the staged `@fuzdev/tsv` loader. Every hand-written code source the
 * loader package copies, not just `index.js`: the staged files are written in one pass,
 * so `index.js`'s mtime dates the whole staging and a sibling edited since then is the
 * same staleness. (`cli.js` gets its own check only because it is the one shared with the
 * wasm package. `README.md` and `LICENSE` are copied too and absent here — a reader that
 * grades the README adds its own check.)
 */
export function napi_loader_checks(): Array<StagedCheck> {
	const rebuild = 'deno task build:napi:packages';
	return [
		{
			label: 'staged loader',
			staged: `${NAPI_LOADER_DIR}/index.js`,
			crates: [],
			files: [
				'crates/tsv_napi/npm/index.js',
				'crates/tsv_napi/npm/index.d.ts',
				'crates/tsv_napi/npm/platform.js',
				'crates/tsv_napi/npm/bin.js',
				...facade_sources(ALL_FAMILIES),
				'scripts/build_napi_packages.ts',
				'scripts/npm_facade.ts',
				'scripts/npm_metadata.ts'
			],
			rebuild
		},
		{
			label: 'staged cli.js mirror',
			staged: `${NAPI_LOADER_DIR}/cli.js`,
			crates: [],
			files: ['crates/tsv_wasm/npm/cli.js'],
			rebuild
		}
	];
}

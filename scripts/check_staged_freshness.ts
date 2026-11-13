/**
 * Freshness guard for the STAGED npm-package artifacts the `:run` test tasks
 * consume — the staged-package sibling of
 * `benches/js/lib/check_artifact_freshness.ts` (the bench/corpus `:run` guard).
 *
 * `npm run test:npm[:parse|:all]` and `npm run test:napi:npm` build first,
 * so what they test is fresh by construction. Their `:run` variants
 * deliberately skip that build — the path for iterating on the test harness —
 * at the risk of silently testing a STALE staging: the incident this guard
 * exists for was `crates/tsv_napi/pkg` holding a native `tsv_cli` binary from
 * before a behavior fix, which `test:napi:npm:run` would have green-tested
 * without a word. `scripts/validate_artifacts.ts` is the third consumer, for
 * the same reason with no `:run` split at all — it never builds, so a bare
 * `npm run validate:artifacts` grades whatever `pkg/` happens to hold.
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
 * of its own (the napi platform package's CLI binary, `engine_parity.ts`'s native binary) adds that check beside the shared list; the platform package's addon has
 * one of its own, `napi_addon_check`, since two suites load it.
 *
 * Staleness here has two lags — the `target/` build behind the sources, and the
 * staged copy behind the build — and comparing the staged file's mtime directly
 * against the SOURCES catches both with one check. That reads a staged file's
 * mtime as when it was staged, which a plain copy does not promise on every
 * platform — the staging scripts copy through `scripts/stage_file.ts`, which does.
 *
 * Each check names the crates and/or individual files that feed one staged
 * artifact; when crates are named, the workspace `Cargo.toml` + `Cargo.lock`
 * are considered too (dependency bumps rebuild the artifact). A missing staged
 * file is always fatal; `BENCH_STALE_OK=1` — the same escape hatch as the
 * bench guard, deliberately one knob for every mtime guard — downgrades a
 * stale (present-but-older) one to a warning. What no mtime check can see is a
 * toolchain change; after one, rebuild once via the build-first task.
 */

import { cpSync, existsSync, mkdirSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { env, exit } from 'node:process';
import { fileURLToPath } from 'node:url';

import {
	CHECKOUT_TREE,
	fmt_mtime,
	newest_source_mtime,
	newest_workspace_file_mtime
} from '../benches/js/lib/check_artifact_freshness.ts';
import type { SourceTree } from '../benches/js/lib/source_mtime.ts';
import { CORE_CRATES, WASM_CRATES } from '../benches/js/lib/crate_lists.ts';
import { wasm_bundle_dir, type WasmVariant } from '../benches/js/lib/tsv_artifacts.ts';
import { ALL_FAMILIES, facade_sources } from './npm_facade.ts';
import { THIRD_PARTY_NOTICE } from './npm_metadata.ts';

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
 * The throws are about the CHECK, not verdicts about the artifact: a `files` entry
 * naming a path that does not exist, and a staged file or source that is there but
 * can't be read.
 * @param tree - the checkout to read, by default the workspace's through `node:fs`
 */
export async function staged_staleness(
	check: StagedCheck,
	tree: SourceTree = CHECKOUT_TREE
): Promise<StagedStaleness | undefined> {
	const staged = await tree.stat(check.staged);
	if (staged === null) {
		return { missing: true, reason: `${check.label}: not staged — ${check.staged}` };
	}
	const staged_ms = staged.mtime_ms;

	let newest = { ms: 0, path: '' };
	if (check.crates.length > 0) {
		newest = { ...(await newest_source_mtime(check.crates, tree)) };
		const workspace = await newest_workspace_file_mtime(tree);
		if (workspace.ms > newest.ms) newest = workspace;
	}
	for (const file of check.files) {
		const st = await tree.stat(file);
		// a named source that does not exist is a typo in the check, and a typo'd
		// path would otherwise degrade the check to no check at all
		if (st === null) {
			throw new Error(`${check.label}: named source ${file} does not exist — fix the check`);
		}
		if (st.mtime_ms > newest.ms) newest = { ms: st.mtime_ms, path: file };
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

/** What `staged_freshness_report` found over a list of checks. */
export interface StagedFreshnessReport {
	/** A staged file is missing, or one is stale and staleness wasn't downgraded. */
	fatal: boolean;
	/** The report, written to stderr as one write: the abort's, or the downgraded warning. */
	message: string;
}

/**
 * Grade every check and return the report `assert_staged_fresh` prints, without printing or
 * exiting, for a caller that reads the checkout through a tree of its own and returns its exit
 * code. `undefined` when everything is fresh.
 * @param options.tree - the checkout to read
 * @param options.stale_ok - `BENCH_STALE_OK=1`: a stale (present but older) file warns
 */
export async function staged_freshness_report(
	checks: readonly StagedCheck[],
	options: { tree: SourceTree; stale_ok: boolean }
): Promise<StagedFreshnessReport | undefined> {
	const findings: string[] = [];
	let missing = false;

	for (const check of checks) {
		const staleness = await staged_staleness(check, options.tree);
		if (!staleness) continue;
		if (staleness.missing) missing = true;
		findings.push(`  • ${staleness.reason}`);
		findings.push(`      restage: ${check.rebuild}`);
	}

	if (findings.length === 0) return undefined;

	const fatal = missing || !options.stale_ok;
	return {
		fatal,
		message: [
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
	};
}

/**
 * Abort (exit 1) when any staged artifact is missing or older than a source
 * feeding it; see the module doc for the escape hatch. Returns normally when
 * everything is fresh (or staleness was downgraded to a warning).
 */
export async function assert_staged_fresh(checks: readonly StagedCheck[]): Promise<void> {
	const report = await staged_freshness_report(checks, {
		tree: CHECKOUT_TREE,
		stale_ok: env.BENCH_STALE_OK === '1'
	});
	if (report === undefined) return;
	console.error(report.message);
	if (report.fatal) exit(1);
}

/**
 * The wasm patcher's sources: its entry, `scripts/roots/patch_npm_package.ts`, and every repo
 * module that entry loads, so an edit to any of them stales what the patcher wrote.
 * `scripts/staging_sources_test.ts` holds the list to the entry's import closure.
 */
export const WASM_STAGING_SOURCES = [
	'scripts/roots/patch_npm_package.ts',
	'scripts/roots/checkout.ts',
	'scripts/patch_npm_package.ts',
	'scripts/npm_facade.ts',
	'scripts/npm_metadata.ts',
	'scripts/stage_file.ts',
	'scripts/stage_file_handle.ts',
	'scripts/gzip_size.ts',
	'scripts/size.ts'
];

/**
 * The N-API staging's sources: its entry, `scripts/roots/build_napi_packages.ts`, and every
 * repo module that entry loads, so an edit to any of them stales what the staging wrote.
 * `scripts/staging_sources_test.ts` holds the list to the entry's import closure.
 */
export const NAPI_STAGING_SOURCES = [
	'scripts/roots/build_napi_packages.ts',
	'scripts/build_napi_packages.ts',
	'scripts/napi_host.ts',
	'scripts/npm_facade.ts',
	'scripts/npm_metadata.ts',
	'scripts/release/version.ts',
	'scripts/stage_file.ts',
	'scripts/size.ts'
];

/** A wasm variant's npm staging (`scripts/patch_npm_package.ts` writes it), repo-relative. */
export function wasm_package_dir(variant: WasmVariant): string {
	return relative(ROOT, wasm_bundle_dir(variant, 'npm'));
}

/**
 * The checks that date one staged wasm package — the one list every reader of a
 * `pkg/<variant>/npm` staging grades it by (the package suite, artifact validation, the
 * napi suite's export-set parity, the engine parity audit, the declaration check, the Bun leg). One check per staging step, keyed
 * on a file that step writes: the bundle (wasm-pack, whose features are a `package.json` task
 * string), the generated entries (the patcher and its inputs), the copied facade, and for
 * the `all` variant the copied `cli.js`.
 */
export function wasm_package_checks(
	variant: WasmVariant,
	dir: string = wasm_package_dir(variant)
): Array<StagedCheck> {
	const rebuild = `npm run build:npm:${variant}`;
	return [
		{
			label: `staged WASM bundle (${variant})`,
			staged: `${dir}/tsv_wasm_bg.wasm`,
			crates: [...CORE_CRATES, ...WASM_CRATES],
			files: ['package.json'],
			rebuild
		},
		{
			// the patcher derives the entries from wasm-pack's generated JS, so whatever feeds
			// that (the crates, the feature set in package.json) feeds them too — a wasm-pack rerun
			// the patcher did not follow must stale them, and `cli.js`, written in the same pass
			label: `staged package entries (${variant})`,
			staged: `${dir}/index.js`,
			crates: [...CORE_CRATES, ...WASM_CRATES],
			// the third-party NOTICE is copied in the same pass
			files: [...WASM_STAGING_SOURCES, 'package.json', THIRD_PARTY_NOTICE.source],
			rebuild
		},
		{
			// every facade file the variant ships — the AST types included — is copied in
			// the same staging step, so the first one's age speaks for the rest
			label: `staged facade (${variant})`,
			staged: `${dir}/api.js`,
			crates: [],
			files: [
				...facade_sources({
					format: variant !== 'parse',
					parse: variant !== 'format',
					strip: variant === 'all',
					imports: variant === 'all'
				}),
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
 * The triple of the one platform package staged beside the loader, from what is staged: whether
 * the loader is, and the entries of `NAPI_PKG_ROOT`. `napi_staged_triple` reads them through
 * `node:fs`; a caller on another tree reads them itself.
 * @returns the triple, or the message naming what is staged when the loader is absent or the
 *   platform packages are not exactly one
 */
export function napi_staged_triple_of(
	loader_staged: boolean,
	entries: ReadonlyArray<string>
): { ok: true; triple: string } | { ok: false; message: string } {
	if (!loader_staged) {
		return {
			ok: false,
			message: `${NAPI_LOADER_DIR} not staged. Run 'npm run build:napi:packages' first.`
		};
	}
	const platform_dirs = entries.filter((d) => d !== 'napi');
	if (platform_dirs.length !== 1) {
		return {
			ok: false,
			message: `expected exactly one staged platform package under ${NAPI_PKG_ROOT}, got: ${platform_dirs.join(', ') || '(none)'}`
		};
	}
	return { ok: true, triple: platform_dirs[0]! };
}

/**
 * The triple of the one platform package staged beside the loader — the BUILD script's
 * host detection, which a loader resolving its sibling by its own detection then agrees
 * with or fails. Exits naming what is staged when the loader is absent or the platform
 * packages are not exactly one.
 */
export function napi_staged_triple(): string {
	const loader_staged = existsSync(`${ROOT}${NAPI_LOADER_DIR}`);
	const staged = napi_staged_triple_of(
		loader_staged,
		loader_staged ? readdirSync(`${ROOT}${NAPI_PKG_ROOT}`) : []
	);
	if (!staged.ok) {
		console.error(staged.message);
		exit(1);
	}
	return staged.triple;
}

/**
 * Lay the staged `@fuzdev/tsv` loader — and, given a triple, that platform package beside
 * it — into `consumer`'s `node_modules/@fuzdev/`, as npm installs them, for a suite that
 * imports the loader by bare specifier. Copies, not links: the loader resolves
 * `@fuzdev/tsv-<triple>` with `createRequire` from its REAL path, which a link would put
 * outside the consumer's `node_modules`.
 *
 * @param consumer - the consumer directory (its `node_modules` is created as needed)
 * @param triple - the platform package to stage beside the loader, or `null` for the loader
 *   alone (the unsupported-platform shape)
 */
export function stage_napi_consumer(consumer: string, triple: string | null): void {
	const scope = join(consumer, 'node_modules', '@fuzdev');
	mkdirSync(scope, { recursive: true });
	cpSync(`${ROOT}${NAPI_LOADER_DIR}`, join(scope, 'tsv'), { recursive: true });
	if (triple !== null) {
		cpSync(`${ROOT}${NAPI_PKG_ROOT}/${triple}`, join(scope, `tsv-${triple}`), { recursive: true });
	}
}

/** The check that dates the staged N-API addon in the platform package for `triple`. */
export function napi_addon_check(triple: string): StagedCheck {
	return {
		label: 'staged N-API addon',
		staged: `${NAPI_PKG_ROOT}/${triple}/tsv_napi.node`,
		// the addon links the discovery crates too (its `format` feature pulls `tsv_ignore` +
		// `tsv_discover` for the `IgnoreStack` export), so an edit there must stale it
		crates: [...CORE_CRATES, 'tsv_napi', 'tsv_ignore', 'tsv_discover'],
		// the third-party NOTICE beside it is staged in the same pass
		files: [...NAPI_STAGING_SOURCES, THIRD_PARTY_NOTICE.source],
		rebuild: 'npm run build:napi:packages'
	};
}

/**
 * The checks that date the staged `@fuzdev/tsv` loader. Every hand-written code source the
 * loader package copies, not just `index.js`: the staged files are written in one pass,
 * so `index.js`'s mtime dates the whole staging and a sibling edited since then is the
 * same staleness. (`cli.js` gets its own check only because it is the one shared with the
 * wasm package. The third-party `NOTICE` is graded with the rest; `README.md` and `LICENSE`
 * are copied too and absent here — a reader that grades the README adds its own check.)
 */
export function napi_loader_checks(): Array<StagedCheck> {
	const rebuild = 'npm run build:napi:packages';
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
				...NAPI_STAGING_SOURCES,
				THIRD_PARTY_NOTICE.source
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

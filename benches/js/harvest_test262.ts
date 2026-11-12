/**
 * Harvest the test262 graded-positive file list for the conformance corpus.
 *
 * Runs `tsv_debug test262 --emit-manifest` (the Rust harness grades tsv's
 * subset from each test's front-matter: `negative:` expectations, the Annex B
 * exclusion, `module` flags), then filters to the EXPECTED-POSITIVE tests and
 * writes the path list the corpus loader's `files_from` entry consumes. The
 * filter is tool-neutral by construction — test262's own metadata decides,
 * never tsv's verdict, so the subset can't bias per-tool coverage toward tsv.
 *
 * Each emitted entry carries its declared parse **goal** (from the manifest's
 * per-file `module` flag: `flags: [module]` → `module`, else `script`), so the
 * conformance-coverage surface parses every tool at the goal test262 declares —
 * a script-goal `await`-identifier test is no longer scored as a failure against
 * a module parse. The manifest carries ONE row per test, and a mode-unflagged
 * test's row is its SLOPPY run, so a `script` entry here is a sloppy script on
 * every tool — the `"use strict"` prefix an `onlyStrict` row would need is the
 * differential's business, not this cache's. (The
 * `diagnostics/test262_compare.ts` differential remains the deeper per-tool
 * goal-aware view.)
 *
 * Output:
 * - benches/js/.cache/test262_files.json — sorted `{path, goal}` array (project-root-relative paths)
 * - benches/js/.cache/test262_manifest.json — the raw manifest (kept for inspection)
 *
 * Flags: `--if-present` tolerates a missing `../test262` checkout (warn +
 * exit 0) — for the `bench:conformance` task chain, where an absent suite
 * should produce a smaller corpus (disclosed via the report's
 * `corpus_sources`), not a failed bench. `--force` re-grades despite a fresh
 * stamp (default runs skip the ~1 min release-mode grade when the ../test262
 * commit + pin match — see `lib/harvest_stamp.ts`). ⚠ The stamp can't see the
 * Rust grading logic: after changing the runner's skip/feature filters, run
 * with `--force` (the graded-manifest pin in `test262.rs` still guards the
 * result either way).
 *
 * Run from the repo root:
 *   npm run bench:harvest:test262 [-- --force]
 *
 * `run_harvest_test262` is the library function; its root, `benches/js/roots/harvest_test262.ts`,
 * binds its deps (`HarvestTest262Deps`).
 *
 * @module
 */

import { join } from 'node:path';

import { TEST262_POSITIVES_PIN } from './lib/gate_counts.ts';
import {
	git_head,
	harvest_up_to_date,
	short_commit,
	type StampGit,
	type StampInputs,
	type StampWrites,
	write_stamp
} from './lib/harvest_stamp.ts';
import { HARVEST_STAMPS } from './lib/harvest_stamp_table.ts';

/** What `run_harvest_test262` reaches the world through. */
export interface HarvestTest262Deps {
	/** `git` for the stamp's checkout commit. */
	git: StampGit;
	/** The checkout's files by project-root-relative path; writes the path list and the stamp. */
	files: StampWrites;
	/** The `../test262` checkout, read for its presence alone. */
	test262: { exists(path: string): Promise<boolean> };
	/**
	 * `cargo` in the checkout, its output inherited: the run that grades the suite and writes
	 * the manifest beneath `benches/js/.cache` itself.
	 */
	cargo: {
		run(
			args: ReadonlyArray<string>
		): Promise<{ readonly kind: string; readonly code?: number | null }>;
	};
}

export const TEST262_ROOT = '../test262';
const CACHE_DIR = 'benches/js/.cache';
export const TEST262_MANIFEST_PATH = `${CACHE_DIR}/test262_manifest.json`;
export const TEST262_FILES_PATH = `${CACHE_DIR}/test262_files.json`;
const STAMP_PATH = HARVEST_STAMPS.test262.path;

/** One manifest row, as `tsv_debug test262 --emit-manifest` writes it. */
export interface Test262ManifestEntry {
	relative_path: string;
	module: boolean;
	strict: boolean;
	expected: 'accept' | 'reject';
	tsv: 'accept' | 'reject';
}

/** The manifest `tsv_debug test262 --emit-manifest` writes. */
export interface Test262Manifest {
	test262_root: string;
	count: number;
	tests: Test262ManifestEntry[];
}

/** One entry of the path list the corpus loader reads. */
export interface Test262File {
	path: string;
	goal: 'module' | 'script';
}

/**
 * The manifest's expected-positive tests as the corpus loader's entries, sorted by path
 * (`localeCompare`, the order the cache has always had): the goal is the manifest's `module`
 * flag (a `flags: [module]` test parses as a module, everything else as a script).
 */
export function test262_positive_files(manifest: Test262Manifest): Test262File[] {
	return manifest.tests
		.filter((t) => t.expected === 'accept')
		.map((t) => ({
			path: join(manifest.test262_root, t.relative_path),
			goal: (t.module ? 'module' : 'script') as 'module' | 'script'
		}))
		.sort((a, b) => a.path.localeCompare(b.path));
}

/**
 * The harvest.
 * @returns the exit code
 */
export async function run_harvest_test262(
	deps: HarvestTest262Deps,
	argv: ReadonlyArray<string>
): Promise<number> {
	const if_present = argv.includes('--if-present');
	const force = argv.includes('--force');

	if (!(await deps.test262.exists('test'))) {
		if (if_present) {
			console.error(
				`test262 checkout not found at ${TEST262_ROOT} — skipping harvest (--if-present)`
			);
			return 0;
		}
		console.error(`test262 checkout not found: ${TEST262_ROOT}`);
		return 1;
	}

	await deps.files.mkdir(CACHE_DIR, { recursive: true });

	// Freshness stamp: skip the release-mode grade when the checkout commit and
	// pin match and both cache files exist.
	const source_commit = await git_head(deps.git, TEST262_ROOT);
	const stamp_inputs: StampInputs = {
		harvest: 'test262',
		source_commit,
		positives_pin: TEST262_POSITIVES_PIN
	};
	if (
		!force &&
		source_commit !== null &&
		(await harvest_up_to_date(deps.files, STAMP_PATH, stamp_inputs, [
			TEST262_FILES_PATH,
			TEST262_MANIFEST_PATH
		]))
	) {
		console.error(
			`test262 harvest up to date (../test262 at ${short_commit(source_commit)}, ` +
				`pin ${TEST262_POSITIVES_PIN}) — skipping; --force to re-grade.`
		);
		return 0;
	}

	// --release: the harness parses the whole ~51k-file tree to grade it; a debug
	// build is an order of magnitude slower.
	console.error('Grading test262 (cargo run --release -p tsv_debug test262 --emit-manifest)…');
	const status = await deps.cargo.run([
		'run',
		'--release',
		'-p',
		'tsv_debug',
		'--',
		'test262',
		'--path',
		TEST262_ROOT,
		'--emit-manifest',
		TEST262_MANIFEST_PATH
	]);
	if (status.kind !== 'exited' || status.code !== 0) {
		console.error('tsv_debug test262 --emit-manifest failed');
		return 1;
	}

	const manifest = JSON.parse(await deps.files.read_text(TEST262_MANIFEST_PATH)) as Test262Manifest;
	const files = test262_positive_files(manifest);

	// Pinned count (exact): the test262 checkout is updated deliberately, so any
	// move in the positive set — a grading/frontmatter bug shrinking it, tsv
	// feature-filter changes, or a checkout pull — must be re-pinned, not
	// absorbed. Fail BEFORE writing the cache so a wrong list never replaces a
	// good one. (The Rust runner separately pins its discovery + graded-manifest
	// counts.) See lib/gate_counts.ts.
	if (files.length !== TEST262_POSITIVES_PIN) {
		console.error(
			`FAIL: pinned count mismatch — ${files.length} expected-positive ≠ pinned ${TEST262_POSITIVES_PIN}; ` +
				`cache not written. If the move is deliberate, re-pin in lib/gate_counts.ts.`
		);
		return 1;
	}
	await deps.files.write(TEST262_FILES_PATH, JSON.stringify(files, null, '\t'));
	if (source_commit !== null) {
		await write_stamp(deps.files, STAMP_PATH, stamp_inputs);
	}

	const module_count = manifest.tests.filter((t) => t.expected === 'accept' && t.module).length;
	console.error(
		`test262 harvest: ${files.length} expected-positive of ${manifest.count} graded tests ` +
			`(${module_count} module-flagged) → ${TEST262_FILES_PATH}`
	);
	return 0;
}

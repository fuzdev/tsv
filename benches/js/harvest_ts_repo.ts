/**
 * Harvest the **tsc corpus** file lists for the conformance surface — the
 * TypeScript-specific parse corpus the surface otherwise lacks.
 *
 * Why it exists: without this entry the conformance `parse/typescript` group is
 * ~95% test262, i.e. ECMAScript, with prettier's ~800 format fixtures as its only
 * TypeScript-specific inputs. `../typescript/tests/cases/{conformance,compiler}`
 * is the language's own test corpus — ~9.4k single-file `.ts` written to exercise
 * TS itself — and it is already a release-required, commit-pinned oracle checkout
 * here (the `conformance:ts-repo` gate reads its baselines).
 *
 * **The validity oracle is tsc itself, run in-process.** The filter must be
 * tool-neutral or the coverage number is rigged, so — exactly like the test262
 * harvest, where test262's own metadata decides and never tsv's verdict — the
 * grader here is the `typescript` npm package's parser. A file is kept only when
 * BOTH readings agree it is well-formed:
 *
 * 1. **No `TS1xxx` in any of its `.errors.txt` baselines** — tsc's *compiler*
 *    reports no grammar error (the `conformance:ts-repo` gate's rule; a
 *    multi-setting test writes per-variant `<name>(target=es5).errors.txt`, so
 *    every variant is indexed).
 * 2. **`createSourceFile(...).parseDiagnostics` is empty** — tsc's *parser* itself
 *    accepts the raw bytes we are about to hand every tool.
 *
 * The two disagree in BOTH directions, which is the whole reason to run tsc rather
 * than trust the baselines alone. Each run REPORTS the size of both disagreements
 * (read them off the final line rather than from prose here, which goes stale on
 * any checkout pull or tsc bump):
 *
 * - Files that pass the baseline rule but fail tsc's parser — deliberately
 *   ill-formed shapes the TS harness compiles from a *virtual* file we don't
 *   reproduce (`shebang.ts` puts `// @target:` above its `#!`, a position error in
 *   the raw bytes), plus private-identifier and exponentiation-LHS errors the
 *   parser raises directly. Non-UTF-8 files (`TS_BINARY_FILE_CODE`) are a third
 *   shape, excluded from both lists.
 * - The much larger reverse: files that fail the baseline rule but parse clean,
 *   because tsc reports many grammar rules (`TS1206` decorators, modifier-order
 *   rules) from CHECKER-side grammar checks rather than from the parser.
 *
 * Keeping only the intersection means every file in the corpus is unambiguously
 * well-formed TypeScript under both readings, so a tool failing one has a real gap
 * — not a disagreement about *when* tsc reports.
 *
 * **Two caches, opposite meanings — never merge them:**
 *
 * - `ts_repo_files.json` — the VALID set, a bare path list (project-root-relative,
 *   like the test262 cache) the corpus loader's `files_from` entry consumes.
 *   Coverage over it reads the usual way (higher is better).
 * - `ts_repo_rejects.json` — files tsc's PARSER rejects. Deliberately NOT a corpus
 *   entry: mixing them into the coverage denominator would make permissiveness read
 *   as superiority, the exact hazard the Svelte canonical-reject exclusion exists to
 *   prevent. It feeds `diagnostics/ts_repo_over_acceptance.ts`, where accepting is
 *   the FAILURE (lower is better). Its membership rule is tsc's parser alone — a
 *   file tsc's parser accepts is not over-acceptance no matter what the checker's
 *   grammar checks say about it later.
 *
 * **No per-file parse goal — deliberately, unlike the test262 entry.** tsc does
 * assign every file a module-vs-script reading (`isExternalModule`), and the
 * obvious move is to emit it the way the test262 harvest emits test262's declared
 * goal. It is wrong here, because the two axes are not the same axis: test262's
 * flag IS the ES `sourceType`, while tsc's reading is a SEMANTIC classification
 * that never gates syntax — tsc parses `import x = M.X` and `export =` in a script
 * exactly as in a module. Feeding that reading to parsers that take `sourceType`
 * as a grammar switch scores them for something tsc doesn't do — and the trade is
 * lopsided. Measured over this corpus (tsc infers `script` for 7,270 of the 8,129,
 * `module` for 859): tagging costs tsv 640 files it and tsc BOTH accept — they
 * fail "'import' is only allowed in a module" — to win back 25 that need script
 * goal (`await` as an identifier), a net 8,016 → 7,401 accepted; acorn moves 7,799
 * → 7,219 the same way. That ~7-point drop would also put this surface at odds
 * with the `conformance:ts-repo` gate over the same corpus for a reason neither
 * documents. So the cache is a bare path list and every file is parsed at the
 * default module goal, like every other non-test262 corpus entry.
 *
 * Skips, every one counted and reported: `.d.ts` (see `DECLARATIONS` — the one place
 * this harvest scopes itself more narrowly than the gate, and why), `.tsx` (JSX is
 * out of scope for tsv), `@filename` multi-file tests (several virtual modules
 * concatenated — not one parse unit), and unreadable files. The parse-unit rule and
 * the baseline-reading rules are SHARED with `diagnostics/ts_repo_compare.ts`
 * (`lib/ts_repo.ts`), so the gate and the bench corpus cannot drift on what a parse
 * unit IS or on what tsc's baselines say about it.
 *
 * Flags: `--if-present` tolerates a missing `../typescript` checkout (warn +
 * exit 0), for the `bench:conformance` task chain. `--force` re-harvests despite a
 * fresh stamp (default runs skip when the checkout commit, the tsc version, and the
 * pins all match — `lib/harvest_stamp.ts`). ⚠ The stamp cannot see THIS file's
 * grading logic: after changing the filter, run with `--force`.
 *
 * Run (from the repo root):
 *   deno run --allow-read --allow-write=benches/js/.cache --allow-env --allow-run=git \
 *     --config benches/js/deno.json benches/js/harvest_ts_repo.ts
 */

import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { basename, join } from 'node:path';

import { TS_REPO_CORPUS_PIN, TS_REPO_REJECTS_PIN } from './lib/gate_counts.ts';
import {
	git_head,
	harvest_up_to_date,
	short_commit,
	type StampInputs,
	write_stamp
} from './lib/harvest_stamp.ts';
import {
	baseline_test_key,
	discover_ts_cases,
	empty_ts_case_skips,
	has_grammar_error,
	is_multi_file_test,
	TS_BASELINE_DIR,
	TS_REPO
} from './lib/ts_repo.ts';
import { load_typescript, tsc_parse } from './lib/tsc.ts';

const ROOTS = [`${TS_REPO}/tests/cases/conformance`, `${TS_REPO}/tests/cases/compiler`];

/**
 * `.d.ts` cases are SKIPPED — a property of this corpus's consumers, not of the
 * files. Everything measured over `ts_repo_files.json` receives file CONTENT under a
 * synthetic `file.ts` name (the bench threads no real path — `lib/tsc.ts`,
 * `lib/perf_omit.ts`), and some syntax is well-formed only *because* the file is a
 * declaration file: `export const browser: boolean;` with no initializer is valid in
 * a `.d.ts` and invalid in a `.ts`. acorn-typescript has no declaration mode at all,
 * oxc and yuku have one they can only reach through the filename. Admitting such a
 * file would score those rows for path plumbing rather than for parsing — the exact
 * rigging the tool-neutral validity filter exists to prevent, and already the cause
 * of every current `PERF_OMITS` entry on the other surface.
 *
 * The cost is nil, which is what makes the rule cheap to hold: of the 40 `.d.ts`
 * under these two roots, 31 carry a `TS1xxx` baseline (ambient-context violations
 * tsc's CHECKER reports — its parser accepts them, so the intersection filter drops
 * them anyway) and the 9 the filter would keep are ~0.1% of an ~8.1k corpus, none of
 * them currently ambient-only. So this guards a future upstream `.d.ts`, not a
 * measurable coverage loss today. The `conformance:ts-repo` GATE reads the same tree
 * and admits them — it grades tsv against tsc alone, where no filename is in play.
 */
const DECLARATIONS = 'skip' as const;

const CACHE_DIR = 'benches/js/.cache';
const FILES_PATH = `${CACHE_DIR}/ts_repo_files.json`;
const REJECTS_PATH = `${CACHE_DIR}/ts_repo_rejects.json`;
const STAMP_PATH = `${CACHE_DIR}/ts_repo.stamp.json`;

/**
 * `TS1490` — "File appears to be binary". An ENCODING verdict, not a grammar one:
 * these files are UTF-16 or latin-1 on disk, so reading them as UTF-8 (what every
 * tool here does) yields replacement characters. Excluded from BOTH lists — a tool
 * that rejects one has no parse gap, and a tool that accepts one is not
 * over-accepting; the file simply isn't a syntax test as we can read it.
 */
const TS_BINARY_FILE_CODE = 1490;

const if_present = Deno.args.includes('--if-present');
const force = Deno.args.includes('--force');

try {
	await stat(join(TS_REPO, 'tests', 'cases'));
} catch {
	if (if_present) {
		console.error(`typescript checkout not found at ${TS_REPO} — skipping harvest (--if-present)`);
		Deno.exit(0);
	}
	console.error(`typescript checkout not found: ${TS_REPO}`);
	Deno.exit(1);
}

const ts = await load_typescript();

await mkdir(CACHE_DIR, { recursive: true });

// Freshness stamp. The tsc version is an input because it IS the oracle: a tsc
// bump can move a file between the two lists without the checkout changing.
const source_commit = git_head(TS_REPO);
const stamp_inputs: StampInputs = {
	harvest: 'ts_repo',
	source_commit,
	tsc_version: ts.version,
	corpus_pin: TS_REPO_CORPUS_PIN,
	rejects_pin: TS_REPO_REJECTS_PIN
};
if (
	!force &&
	source_commit !== null &&
	(await harvest_up_to_date(STAMP_PATH, stamp_inputs, [FILES_PATH, REJECTS_PATH]))
) {
	console.error(
		`ts-repo harvest up to date (${TS_REPO} at ${short_commit(source_commit)}, ` +
			`tsc ${ts.version}, pins ${TS_REPO_CORPUS_PIN}/${TS_REPO_REJECTS_PIN}) — ` +
			`skipping; --force to re-grade.`
	);
	Deno.exit(0);
}

// Index every `*.errors.txt` baseline by its un-suffixed test name, so a lookup
// gathers all target/module variants (the rule + why: lib/ts_repo.ts).
let baseline_names: string[];
try {
	baseline_names = await readdir(TS_BASELINE_DIR);
} catch (e) {
	console.error(
		`FAIL: cannot read ${TS_BASELINE_DIR} (${e instanceof Error ? e.message : e}) — ` +
			`the ${TS_REPO} checkout exists but its baselines are missing (partial checkout?).`
	);
	Deno.exit(1);
}
const grammar_error_tests = new Set<string>();
for (const name of baseline_names) {
	if (!name.endsWith('.errors.txt')) continue;
	const key = baseline_test_key(name);
	if (grammar_error_tests.has(key)) continue;
	if (has_grammar_error(await readFile(join(TS_BASELINE_DIR, name), 'utf8'))) {
		grammar_error_tests.add(key);
	}
}

const valid: string[] = [];
const rejects: string[] = [];
const discovery_skips = empty_ts_case_skips();
const skipped = { multi_file: 0, binary: 0, unreadable: 0 };
let baseline_rejected = 0;
/**
 * Rejects with no `TS1xxx` baseline — the two validity readings disagreeing in the
 * direction the baselines alone can't show (`baseline_rejected` is the other). Both
 * are reported because the size of each is the argument for running tsc at all.
 */
let rejected_without_baseline = 0;
let scanned = 0;

for (const root of ROOTS) {
	for await (const path of discover_ts_cases(root, discovery_skips, DECLARATIONS)) {
		let content: string;
		try {
			content = await readFile(path, 'utf8');
		} catch {
			skipped.unreadable++;
			continue;
		}
		if (is_multi_file_test(content)) {
			skipped.multi_file++;
			continue;
		}
		scanned++;
		const { diagnostics } = tsc_parse(ts, path, content);
		if (!diagnostics) {
			console.error(
				'FAIL: tsc SourceFile has no `parseDiagnostics` — the internal field this harvest ' +
					'grades with is gone (upstream rename?). Refusing to write a corpus every file ' +
					'would pass into.'
			);
			Deno.exit(1);
		}
		if (diagnostics.some((d) => d.code === TS_BINARY_FILE_CODE)) {
			skipped.binary++;
			continue;
		}
		const has_baseline_error = grammar_error_tests.has(basename(path, '.ts'));
		if (diagnostics.length > 0) {
			if (!has_baseline_error) rejected_without_baseline++;
			rejects.push(path);
			continue;
		}
		// tsc's parser accepts. The baselines get the last word on whether the file
		// is well-formed TS: a checker-side grammar error (TS1206 and friends) keeps
		// it out of the coverage denominator without making it over-acceptance bait.
		if (has_baseline_error) {
			baseline_rejected++;
			continue;
		}
		valid.push(path);
	}
}

valid.sort((a, b) => a.localeCompare(b));
rejects.sort((a, b) => a.localeCompare(b));

// Pinned counts (exact), checked BEFORE writing so a wrong list never replaces a
// good one. Both move only on a deliberate input change — a checkout pull, a tsc
// bump, or a grading change here. See lib/gate_counts.ts.
const mismatches: string[] = [];
if (valid.length !== TS_REPO_CORPUS_PIN) {
	mismatches.push(`valid ${valid.length} ≠ pinned ${TS_REPO_CORPUS_PIN}`);
}
if (rejects.length !== TS_REPO_REJECTS_PIN) {
	mismatches.push(`rejects ${rejects.length} ≠ pinned ${TS_REPO_REJECTS_PIN}`);
}
if (mismatches.length > 0) {
	console.error(
		`FAIL: pinned count mismatch — ${mismatches.join(', ')}; caches not written. ` +
			`If the move is deliberate (checkout pull, tsc bump, grading change), re-pin in ` +
			`lib/gate_counts.ts.`
	);
	Deno.exit(1);
}

await writeFile(FILES_PATH, JSON.stringify(valid, null, '\t') + '\n');
await writeFile(REJECTS_PATH, JSON.stringify(rejects, null, '\t') + '\n');
if (source_commit !== null) {
	await write_stamp(STAMP_PATH, stamp_inputs);
}

// Every bucket, so the two lists always account for the whole scan:
// scanned = valid + rejects + baseline_rejected + binary. The two DISAGREEMENT
// counts (`baseline_rejected`, `rejected_without_baseline`) are the measurement
// that justifies running tsc's parser instead of trusting the baselines alone —
// quote them from here rather than hand-maintaining them in a docstring.
console.error(
	`ts-repo harvest (tsc ${ts.version}): ${valid.length} valid → ${FILES_PATH}; ` +
		`${rejects.length} tsc-parser rejects → ${REJECTS_PATH}. ` +
		`Of ${scanned} single-file .ts scanned: ${baseline_rejected} dropped by a TS1xxx baseline ` +
		`(tsc's parser accepted, its grammar checks did not), ${rejected_without_baseline} rejected ` +
		`with no TS1xxx baseline (the other direction), ${skipped.binary} non-UTF-8. ` +
		`Skipped before grading: ${skipped.multi_file} @filename multi-file, ` +
		`${discovery_skips.tsx} .tsx, ${discovery_skips.declaration} .d.ts, ` +
		`${skipped.unreadable} unreadable.`
);

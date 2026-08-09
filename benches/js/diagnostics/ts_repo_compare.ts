/**
 * TypeScript-repo parse-conformance triage — runs tsv's TS parser against the
 * official `microsoft/typescript` compiler's own parser test corpus
 * (`../typescript/tests/cases` — the WHOLE corpus, ~13.7k single-file `.ts`), using
 * **tsc's own baselines as the validity oracle** (L1) instead of a live parser.
 *
 * Why tsc baselines, not acorn: acorn-typescript is tsv's drop-in *target* but is
 * itself over-lenient (it accepts invalid TS the real compiler rejects), so it's
 * an imperfect *validity* oracle. tsc is authoritative. The TS test harness writes
 * `tests/baselines/reference/<name>.errors.txt` iff a compile produces diagnostics
 * (a multi-setting test writes per-variant `<name>(target=es5).errors.txt` instead —
 * see `errors_baselines_by_test`, which indexes every variant); a `TS1xxx` code there
 * is a **syntax/grammar** error (tsc's parser rejects), while `TS2xxx`+ are semantic
 * (tsc's parser accepted). So:
 *
 *   tsc-syntax-valid(file)  ⇔  no `.errors.txt` in any variant, or none of their codes is TS1xxx.
 *
 * Buckets (tsv verdict × tsc validity), with acorn's verdict as a sub-label:
 *
 *   - `accept_parity`  — tsv accepts, tsc-valid. Healthy.
 *   - `reject_parity`  — tsv rejects, tsc-INVALID (TS1xxx). Healthy: tsv is
 *                        correctly stricter and matches tsc — this is where the
 *                        acorn-over-leniency cases land (acorn accepts, but tsc and
 *                        tsv both reject), so they need NO sanction here.
 *   - `over_acceptance`— tsv accepts, tsc-invalid. tsv's documented lenient posture
 *                        (deferred early-errors). Reported, not gated.
 *   - over-rejection   — tsv rejects, tsc-VALID: a real tsv parse gap. Sub-split by
 *                        acorn:
 *                          · `gap` (acorn also accepts) — high-confidence: BOTH
 *                            oracles say valid. Classified vs KNOWN_GAPS; an
 *                            un-tracked one GATES (exit 1).
 *                          · `gap_beyond_acorn` (acorn also rejects) — tsv matches
 *                            its acorn target; only tsc is more permissive. A MIXED
 *                            surface needing manual triage, because "no TS1xxx" ≠
 *                            "a strict parser should accept": tsc's parser is
 *                            error-recovering and defers many EARLY ERRORS to the
 *                            checker (TS2xxx), so this bucket blends (i) genuine
 *                            acorn-ts parser gaps (e.g. `>>`/generic ambiguity) with
 *                            (ii) early-error TIMING — where tsv+acorn correctly
 *                            reject at parse (`(foo()) = x`, `for (foo() in b)`) what
 *                            tsc flags later as TS2364/TS2406. Only (i) is a real
 *                            gap, and fixing it diverges tsv from acorn toward spec.
 *                            Reported, never gated.
 *
 * Scope: single-file sources, `.d.ts` INCLUDED (see `DECLARATIONS`) — `.tsx` (JSX
 * grammar) and `@filename` multi-file tests are skipped and counted (feeding a
 * multi-module test as one parse unit is meaningless). Discovery and the
 * baseline-reading rules are SHARED with the tsc-corpus harvest (`lib/ts_repo.ts`),
 * so the gate and the bench corpus cannot drift on what a parse unit IS.
 * Compiler-directive comments (`// @target`, `// @strict`) are inert to a parser
 * (they're `//` comments) so nothing is stripped.
 *
 * SEPARATE from the acorn-typescript-suite gate (`ts_fixtures_compare.ts`): that
 * gate's oracle is the live acorn parser over acorn's OWN curated `test/` suite;
 * this tool's oracle is tsc's baselines over the official compiler corpus. They
 * track different gaps against different oracles, so their KNOWN_GAPS lists are
 * deliberately kept apart.
 *
 * In the blocking `conformance` aggregate (publish Step 3b) since its baseline
 * hit 0 untracked gaps — exits 1 on an un-tracked `gap`. Summary to stderr,
 * full JSON to stdout. `run_ts_repo_compare` is the importable entry the
 * `conformance.ts` single-process driver calls; the CLI guard at the bottom
 * feeds it `Deno.args`. Failure semantics are process-level (`Deno.exit(1)`),
 * matching the old `&&`-chain aggregate exactly.
 *
 * Setup posture: strict — a missing `../typescript` checkout, a PARTIAL checkout
 * (baselines or the corpus subtree missing), or an empty scan all FAIL rather
 * than green-skipping (the baselines are the oracle; publish Step 3b's preflight
 * probe is the tolerance point for machines without the checkout). Full-corpus
 * runs also freshness-check KNOWN_GAPS (an entry matching nothing fails — delete
 * it when its gap is fixed).
 *
 * Run (from the repo root):
 *   deno task conformance:ts-repo            # builds corpus FFI, then runs
 *   deno task conformance:ts-repo:run        # skip rebuild (freshness-guarded)
 *   deno task conformance:ts-repo:run --json 2>/dev/null > report.json
 *   deno task conformance:ts-repo:run ../typescript/tests/cases/conformance/parser/ecmascript6
 */

import { readdir, readFile, stat } from 'node:fs/promises';
import { basename, join } from 'node:path';

import { init_compare_implementations } from '../lib/compare_cli.ts';
import { TS_REPO_PINS } from '../lib/gate_counts.ts';
import type { KnownGap } from '../lib/parse_sanctions.ts';
import {
	baseline_test_key,
	discover_ts_cases,
	empty_ts_case_skips,
	has_grammar_error,
	is_multi_file_test,
	TS_BASELINE_DIR,
	TS_REPO
} from '../lib/ts_repo.ts';

/**
 * The WHOLE corpus, not the `conformance/parser` subtree the gate originally scanned.
 *
 * The narrow root was chosen on the reasoning that the parser tests are where parser
 * bugs live. That reasoning is what kept **33 over-rejections hidden** across eight
 * grammar families: the newly-swept trees are tsc's *checker and emitter* tests —
 * syntactically ordinary TS written to trip semantic rules — and a parse gap surfacing
 * in shallow test code is likelier reachable in real code than one buried in the parser
 * torture suite.
 *
 * ⚠️ **33 and 32 are different quantities, not a typo.** 33 is the total the widened
 * root exposed; one family (a for-in with an expression left side inside a grouping)
 * landed on its own first, so the tree this gate's own re-pin was measured against
 * already carried its fix and showed **32**. Both numbers are therefore real, and both
 * get quoted. Measured: the narrow root was green at 768 files while the full root
 * carried 32 untracked gaps, all 32 fixed in the same change that widened the root.
 *
 * Scanning everything rather than `conformance` + `compiler` is deliberate: the full root
 * is a strict superset adding 4,238 files (`fourslash`, `projects`, `transpile`,
 * `unittests`) and **zero** new gaps, so the narrower pair would only be a second thing to
 * keep right. Cost, measured warm on an otherwise-quiet machine: ~0.6 s for the narrow
 * root against ~2.7 s for the whole one, so widening buys those trees for **~+2 s** on a
 * `conformance` aggregate measured in tens of seconds — not the "1–2 min" this was once
 * believed to cost. ⚠️ That wall clock moves several-fold with machine load (the same run
 * takes ~5.5 s at load average 5), so re-measure before quoting rather than trusting a
 * number recorded mid-session.
 *
 * ⚠️ The tsc-corpus **bench harvest** (`harvest_ts_repo.ts`) deliberately keeps the
 * narrower `conformance` + `compiler` pair. That is not drift: it is building a *corpus of
 * valid TS* to measure coverage on, where the extra trees add fixture noise, while this
 * gate is hunting *over-rejections*, where a superset can only help. `lib/ts_repo.ts` is
 * what the two must agree on — the parse-unit rule and the baseline rules — not the scope.
 */
const DEFAULT_ROOT = `${TS_REPO}/tests/cases`;

/**
 * `.d.ts` cases are GRADED here, by the same argument that took the root to the whole
 * corpus: this gate hunts over-rejections, and a declaration file is ordinary
 * TypeScript to tsv — no declaration mode exists, `tsv format` discovers a `.d.ts`
 * like any other `.ts`, so a parse gap in one ships. Excluding them was never argued
 * on its own; it rode along with `.tsx`, which has a real reason (JSX is out of
 * scope). The bench harvest keeps skipping them for a reason of its OWN — its
 * consumers get content with no path (`harvest_ts_repo.ts` `DECLARATIONS`).
 *
 * Nothing else moves to admit them: `baseline_test_key` already keys `<name>.d.ts` to
 * its `<name>.d.errors.txt` baseline, so the oracle reads them unchanged.
 *
 * ⚠️ They do sharpen a pre-existing soft spot in the oracle. "No `.errors.txt` ⇒
 * tsc-valid" holds only where the harness compiles each case and baselines it; under
 * `tests/cases/projects/` a DIFFERENT harness compiles whole scenarios, writing
 * `baselines/reference/project/<Scenario>/{amd,node}/` baselines keyed on the scenario
 * and only for its declared `inputFiles` — so a file no scenario names is silently
 * ungraded, and absence there means "nobody asked", not "tsc accepts". That is
 * `projects/declarations_GlobalImport/useModule.d.ts`, a stale TS-1.x emit artifact
 * (`import x = module("y")`, syntax removed years ago) that no test compiles: tsc's
 * parser rejects it (TS1005, confirmed in-process), acorn rejects it, and tsv rejects
 * it correctly — but the baseline rule calls it valid, so it lands in the mixed
 * `gap_beyond_acorn` triage bucket rather than in `reject_parity`. Reported, never
 * gated, which is why one mislabeled entry is tolerable and a special case is not; the
 * 23 no-baseline files already in that bucket sit there on the same technicality.
 */
const DECLARATIONS = 'include' as const;

/**
 * Whether `root` is the default corpus, for the full-corpus-only hygiene block.
 *
 * Normalized rather than compared with `===`: now that the default is a path a person
 * plausibly types by hand, a trailing slash (`…/tests/cases/`) would silently skip BOTH
 * the stale-`KNOWN_GAPS` check and the `TS_REPO_PINS` check — a run that looks like the
 * full gate but grades strictly less.
 */
function is_default_root(root: string): boolean {
	const strip = (p: string) => p.replace(/\/+$/, '');
	return strip(root) === strip(DEFAULT_ROOT);
}

/**
 * tsv parse gaps confirmed against BOTH oracles (tsc-valid AND acorn-accepts),
 * tracked so the tool is green at baseline and a NEW gap surfaces. Must only
 * SHRINK: delete an entry when its gap is fixed. Kept SEPARATE from
 * `ts_fixtures_compare.ts` KNOWN_GAPS (different corpus + oracle). Full triage lives
 * in internal notes. `pattern` uses `<basename>.ts` to avoid numeric-suffix collisions
 * (`…Declaration1` vs `…Declaration11`).
 */
const KNOWN_GAPS: KnownGap[] = [
	// Empty: Gap C (accessor/method bodies in a `declare` class) fixed — ambient
	// members now parse a `{` body like concrete members (acorn/tsc accept). A new
	// tsc-accepted over-rejection surfaces here as an untracked `gap` (exits 1).
];

function first_line(e: unknown): string {
	return String(e instanceof Error ? e.message : e).split('\n')[0];
}

interface Gap {
	path: string;
	tsv_error: string;
	category?: string;
	reason?: string;
}

/** The tool's entry — see the module docstring for buckets, posture, and CLI use. */
export async function run_ts_repo_compare(argv: string[] = Deno.args): Promise<void> {
	const flags = new Set(argv.filter((a) => a.startsWith('-')));
	const json_mode = flags.has('--json');
	const verbose = flags.has('--verbose') || flags.has('-v');
	const root = argv.find((a) => !a.startsWith('-')) ?? DEFAULT_ROOT;

	// A run that can't grade anything is a failure, not a pass — checked before
	// any other work so the error is this message, not a raw ENOENT stack trace.
	// The tolerance point for machines without the checkout is publish Step 3b's
	// preflight probe, which skips the whole aggregate with a warning.
	try {
		await stat(TS_REPO);
	} catch {
		console.error(
			`FAIL: ${TS_REPO} checkout not found — nothing can be graded. ` +
				`Clone microsoft/TypeScript at ${TS_REPO}.`
		);
		Deno.exit(1);
	}

	// Index of every `*.errors.txt` baseline, keyed by its **un-suffixed** test name
	// (`baseline_test_key`, shared with the tsc-corpus harvest — most corpus files are
	// compiled under multiple settings and so carry only per-variant baselines).
	// Index once so a lookup gathers every variant.
	const errors_baselines_by_test = new Map<string, string[]>();
	let baseline_names: string[];
	try {
		baseline_names = await readdir(TS_BASELINE_DIR);
	} catch (e) {
		console.error(
			`FAIL: cannot read ${TS_BASELINE_DIR} (${e instanceof Error ? e.message : e}) — ` +
				`the ${TS_REPO} checkout exists but its baselines are missing (partial/sparse checkout?). ` +
				`The baselines ARE the oracle, so this run cannot grade anything.`
		);
		Deno.exit(1);
	}
	for (const name of baseline_names) {
		if (!name.endsWith('.errors.txt')) continue;
		const key = baseline_test_key(name);
		(errors_baselines_by_test.get(key) ?? errors_baselines_by_test.set(key, []).get(key)!).push(
			name
		);
	}

	/** tsc's parser verdict for a test, read from its baseline(s) (all target/module variants). */
	async function tsc_syntax_valid(file_path: string): Promise<boolean> {
		const base = basename(file_path).replace(/\.ts$/, '');
		const baselines = errors_baselines_by_test.get(base);
		// No `.errors.txt` in any variant → the compile was clean → tsc accepts.
		if (!baselines?.length) return true;
		// A TS1xxx code = a syntax/grammar diagnostic → tsc's parser rejects. If ANY
		// variant carries one, tsc rejects (grammar errors are target-independent).
		for (const name of baselines) {
			if (has_grammar_error(await readFile(join(TS_BASELINE_DIR, name), 'utf8'))) return false;
		}
		return true;
	}

	const { canonical, native } = await init_compare_implementations();

	const buckets = {
		accept_parity: 0,
		reject_parity: 0,
		// Paths only: tsv ACCEPTED these, so there is no tsv-side error to carry. The
		// sibling gates put the ORACLE's rejection message in that slot; tsc's lives in
		// a `.errors.txt` this tool reads as a boolean, so there is nothing to pass on.
		over_acceptance: [] as { path: string }[],
		gap_known: [] as Gap[],
		gap_unexpected: [] as Gap[],
		gap_beyond_acorn: [] as { path: string; tsv_error: string }[]
	};
	// `.tsx` is filtered (and counted) inside discovery, which admits `.d.ts` here
	// (`DECLARATIONS`); the rest are content-level decisions this loop makes.
	const discovery_skips = empty_ts_case_skips();
	const skipped = { multi_file: 0, unreadable: 0 };
	// Reported, not skipped: `.d.ts` is graded here (`DECLARATIONS`), and a run that
	// shows only its skips can't show that. Counted post-content so it matches the
	// graded population, not the discovered one.
	let declarations_graded = 0;
	// Ledger-freshness tracking (see the stale check at the end).
	const used_gaps = new Set<string>();

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
		if (path.endsWith('.d.ts')) declarations_graded++;

		let tsv_err: string | null = null;
		try {
			native.parse_internal(content, 'typescript');
		} catch (e) {
			tsv_err = first_line(e);
		}
		const tsc_valid = await tsc_syntax_valid(path);

		if (!tsv_err) {
			if (tsc_valid) buckets.accept_parity++;
			else buckets.over_acceptance.push({ path });
			continue;
		}
		// tsv rejects.
		if (!tsc_valid) {
			buckets.reject_parity++;
			continue;
		}
		// tsv rejects but tsc accepts → a gap. Sub-label by acorn's verdict.
		let acorn_ok = true;
		try {
			canonical.parse(content, 'typescript');
		} catch {
			acorn_ok = false;
		}
		if (!acorn_ok) {
			buckets.gap_beyond_acorn.push({ path, tsv_error: tsv_err });
			continue;
		}
		const gap = KNOWN_GAPS.find((g) => path.includes(g.pattern));
		if (gap) {
			used_gaps.add(gap.pattern);
			buckets.gap_known.push({
				path,
				tsv_error: tsv_err,
				category: gap.category,
				reason: gap.reason
			});
		} else {
			buckets.gap_unexpected.push({ path, tsv_error: tsv_err });
		}
	}

	canonical.dispose();
	native.dispose();

	// --- Report -----------------------------------------------------------------

	const gap_by_category = new Map<string, number>();
	for (const g of buckets.gap_known) {
		gap_by_category.set(g.category!, (gap_by_category.get(g.category!) ?? 0) + 1);
	}

	const scanned =
		buckets.accept_parity +
		buckets.reject_parity +
		buckets.over_acceptance.length +
		buckets.gap_known.length +
		buckets.gap_unexpected.length +
		buckets.gap_beyond_acorn.length;

	// The checkout exists (guarded above), so an empty scan means a wrong subtree
	// path or a gutted corpus — a broken invocation, not a pass.
	if (scanned === 0) {
		console.error(
			`FAIL: 0 single-file .ts scanned under ${root} — wrong path? Nothing was graded.`
		);
		Deno.exit(1);
	}

	console.error(`\nTypeScript-repo parse-conformance triage — root: ${root}`);
	console.error(`  oracle: tsc baselines (${TS_BASELINE_DIR})`);
	console.error(
		`  scanned: ${scanned} single-file .ts, incl. ${declarations_graded} .d.ts  ` +
			`(skipped ${skipped.multi_file} @filename, ${discovery_skips.tsx} .tsx, ` +
			`${skipped.unreadable} unreadable)\n`
	);
	console.error(`  parity accept (tsv ok, tsc valid):     ${buckets.accept_parity}`);
	console.error(
		`  parity reject (tsv + tsc both reject): ${buckets.reject_parity}  (incl. acorn-over-leniency — no sanction needed)`
	);
	console.error(
		`  over-acceptance (tsv ok, tsc invalid): ${buckets.over_acceptance.length}  (deferred early-errors; not gated)`
	);
	console.error(
		`  GAPS known (tsc+acorn valid, tsv rejects): ${buckets.gap_known.length}  (${
			[...gap_by_category.entries()].map(([c, n]) => `${c}=${n}`).join(', ') || 'none'
		})`
	);
	console.error(
		`  gap-beyond-acorn (tsc valid, acorn+tsv reject): ${buckets.gap_beyond_acorn.length}  (MIXED: acorn-ts gaps + early-error timing; manual triage; not gated)`
	);
	console.error(`  GAPS UNEXPECTED (untracked): ${buckets.gap_unexpected.length}  (GATES)`);

	for (const g of buckets.gap_unexpected) {
		console.error(`\n    ✗ UNEXPECTED gap: ${g.path}\n        ${g.tsv_error}`);
	}
	if (verbose) {
		for (const g of buckets.gap_known) {
			console.error(`      · known-gap [${g.category}] ${basename(g.path)} — ${g.reason}`);
		}
		for (const g of buckets.gap_beyond_acorn) {
			console.error(`      · beyond-acorn ${basename(g.path)} — ${g.tsv_error}`);
		}
		for (const g of buckets.over_acceptance) {
			console.error(`      · over-acceptance ${basename(g.path)}`);
		}
	}

	if (json_mode) {
		const report = {
			root,
			oracle: 'tsc-baselines',
			scanned,
			declarations_graded,
			skipped: { ...skipped, ...discovery_skips },
			accept_parity: buckets.accept_parity,
			reject_parity: buckets.reject_parity,
			over_acceptance: buckets.over_acceptance,
			gap_known: buckets.gap_known,
			gap_known_by_category: Object.fromEntries(gap_by_category),
			gap_beyond_acorn: buckets.gap_beyond_acorn,
			gap_unexpected: buckets.gap_unexpected
		};
		Deno.stdout.writeSync(new TextEncoder().encode(JSON.stringify(report, null, '\t') + '\n'));
	}

	if (buckets.gap_unexpected.length > 0) {
		console.error(
			`\nFAIL: ${buckets.gap_unexpected.length} untracked gap(s) — tsv rejects input BOTH tsc and acorn accept. ` +
				`Fix the parser, or add a reasoned KNOWN_GAPS entry (this file, separate from the acorn-suite gate).`
		);
		Deno.exit(1);
	}

	// Full-corpus-only hygiene (a subtree run legitimately grades a slice):
	if (is_default_root(root)) {
		// Ledger freshness: a KNOWN_GAPS entry matching nothing means its gap was
		// fixed (delete it) or upstream renamed the test (update it) — the list must
		// mirror the live corpus, like scan_audit's ALLOW list.
		const stale = KNOWN_GAPS.filter((g) => !used_gaps.has(g.pattern));
		if (stale.length > 0) {
			console.error(
				`\nFAIL: ${stale.length} stale KNOWN_GAPS entr${stale.length === 1 ? 'y' : 'ies'} — matched no over-rejection:\n` +
					stale.map((g) => `    · ${g.pattern}`).join('\n')
			);
			Deno.exit(1);
		}

		// Pinned counts (exact): the corpus is a deliberately-updated checkout, so any
		// move — a shrunken corpus, a collapsed oracle (accept-parity draining into
		// reject-parity/over-acceptance), or a tsv behavior change — must be re-pinned
		// deliberately, never absorbed.
		const pin_failures = [
			scanned !== TS_REPO_PINS.scanned
				? `scanned ${scanned} ≠ pinned ${TS_REPO_PINS.scanned}`
				: null,
			buckets.accept_parity !== TS_REPO_PINS.accept_parity
				? `accept-parity ${buckets.accept_parity} ≠ pinned ${TS_REPO_PINS.accept_parity}`
				: null,
			// The WIDENING axis, which the two above structurally cannot see: together they
			// fix how many tsc-VALID files tsv accepts, leaving the reject / over-accept /
			// beyond-acorn split of the remainder free. So a fix that ALSO starts accepting
			// something tsc rejects moves only this number, and nothing reports it. Being
			// reported-not-gated is right for the bucket's CONTENT (a deferred early error is
			// policy, not a bug); its SIZE still has to move deliberately.
			buckets.over_acceptance.length !== TS_REPO_PINS.over_acceptance
				? `over-acceptance ${buckets.over_acceptance.length} ≠ pinned ${TS_REPO_PINS.over_acceptance}`
				: null
		].filter((f): f is string => f !== null);
		if (pin_failures.length > 0) {
			console.error(
				`\nFAIL: pinned count mismatch — ${pin_failures.join('; ')}. If this move is deliberate ` +
					`(checkout pull, behavior change), re-pin in lib/gate_counts.ts (see its update ritual).`
			);
			Deno.exit(1);
		}
	}

	console.error(
		`\nOK: no untracked gaps (all tsv over-rejections are tracked or acorn-confirmed-invalid).`
	);
}

if (import.meta.main) {
	await run_ts_repo_compare();
}

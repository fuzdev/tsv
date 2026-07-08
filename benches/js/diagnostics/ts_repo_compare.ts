/**
 * TypeScript-repo parse-conformance triage — runs tsv's TS parser against the
 * official `microsoft/typescript` compiler's own parser test corpus
 * (`../typescript/tests/cases/conformance/parser`, ~800 single-file `.ts`), using
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
 *                            gap, and fixing it diverges tsv from acorn toward spec
 *                            (see ./TODO_UPSTREAM.md). Reported, never gated.
 *
 * Scope: single-file `.ts` only — `.tsx` (JSX grammar) and `@filename` multi-file
 * tests are skipped and counted (feeding a multi-module test as one parse unit is
 * meaningless). Compiler-directive comments (`// @target`, `// @strict`) are inert
 * to a parser (they're `//` comments) so nothing is stripped.
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

/** The official TypeScript checkout (baselines live at a fixed path under it). */
const TS_REPO = '../typescript';
const BASELINE_DIR = `${TS_REPO}/tests/baselines/reference`;
const DEFAULT_ROOT = `${TS_REPO}/tests/cases/conformance/parser`;

/**
 * tsv parse gaps confirmed against BOTH oracles (tsc-valid AND acorn-accepts),
 * tracked so the tool is green at baseline and a NEW gap surfaces. Must only
 * SHRINK: delete an entry when its gap is fixed. Kept SEPARATE from
 * `ts_fixtures_compare.ts` KNOWN_GAPS (different corpus + oracle). Full triage:
 * internal notes, TODO_PARSE_COVERAGE.md §"Broadening — the official typescript repo".
 * `pattern` uses `<basename>.ts` to avoid numeric-suffix collisions
 * (`…Declaration1` vs `…Declaration11`).
 */
const KNOWN_GAPS: KnownGap[] = [
	// Empty: Gap C (accessor/method bodies in a `declare` class) fixed — ambient
	// members now parse a `{` body like concrete members (acorn/tsc accept). A new
	// tsc-accepted over-rejection surfaces here as an untracked `gap` (exits 1).
];

async function* discover(dir: string): AsyncGenerator<string> {
	let entries;
	try {
		entries = await readdir(dir, { withFileTypes: true });
	} catch (e) {
		console.error(`Cannot read ${dir}: ${e instanceof Error ? e.message : e}`);
		return;
	}
	for (const entry of entries) {
		const full = join(dir, entry.name);
		if (entry.isDirectory()) {
			if (entry.name !== 'node_modules') yield* discover(full);
		} else if (entry.name.endsWith('.ts') && !entry.name.endsWith('.d.ts')) {
			yield full;
		}
	}
}

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
				`Clone microsoft/TypeScript at ${TS_REPO}.`,
		);
		Deno.exit(1);
	}

	// Index of every `*.errors.txt` baseline, keyed by its **un-suffixed** test name.
	// A test compiled under multiple settings (`// @target: es5, es2015`, `@module`, …)
	// writes per-variant baselines `<name>(target=es5).errors.txt` rather than a plain
	// `<name>.errors.txt` — ~700 of the ~768 corpus files carry such a directive. Reading
	// only `<name>.errors.txt` therefore MISSES the suffixed baselines and mis-reads a
	// tsc grammar rejection as a clean compile (e.g. `parserAccessors5` → TS1183 lives in
	// `parserAccessors5(target=es5).errors.txt`). Index once so a lookup gathers every
	// variant. Key = filename minus the trailing `(…)` group(s) and `.errors.txt`.
	const errors_baselines_by_test = new Map<string, string[]>();
	let baseline_names: string[];
	try {
		baseline_names = await readdir(BASELINE_DIR);
	} catch (e) {
		console.error(
			`FAIL: cannot read ${BASELINE_DIR} (${e instanceof Error ? e.message : e}) — ` +
				`the ${TS_REPO} checkout exists but its baselines are missing (partial/sparse checkout?). ` +
				`The baselines ARE the oracle, so this run cannot grade anything.`,
		);
		Deno.exit(1);
	}
	for (const name of baseline_names) {
		if (!name.endsWith('.errors.txt')) continue;
		const key = name.replace(/\.errors\.txt$/, '').replace(/\(.*\)$/, '');
		(errors_baselines_by_test.get(key) ?? errors_baselines_by_test.set(key, []).get(key)!).push(
			name,
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
			const errors = await readFile(join(BASELINE_DIR, name), 'utf8');
			if (/error TS1\d{3}:/.test(errors)) return false;
		}
		return true;
	}

	const { canonical, native } = await init_compare_implementations();

	const buckets = {
		accept_parity: 0,
		reject_parity: 0,
		over_acceptance: [] as { path: string; tsv_error: string }[],
		gap_known: [] as Gap[],
		gap_unexpected: [] as Gap[],
		gap_beyond_acorn: [] as { path: string; tsv_error: string }[],
	};
	const skipped = { tsx: 0, multi_file: 0, unreadable: 0 };
	// Ledger-freshness tracking (see the stale check at the end).
	const used_gaps = new Set<string>();

	for await (const path of discover(root)) {
		if (path.endsWith('.tsx')) {
			skipped.tsx++;
			continue;
		}
		let content: string;
		try {
			content = await readFile(path, 'utf8');
		} catch {
			skipped.unreadable++;
			continue;
		}
		// Multi-file tests concatenate several virtual modules — not one parse unit.
		if (/\/\/\s*@[Ff]ilename:/.test(content)) {
			skipped.multi_file++;
			continue;
		}

		let tsv_err: string | null = null;
		try {
			native.parse_internal(content, 'typescript');
		} catch (e) {
			tsv_err = first_line(e);
		}
		const tsc_valid = await tsc_syntax_valid(path);

		if (!tsv_err) {
			if (tsc_valid) buckets.accept_parity++;
			else buckets.over_acceptance.push({ path, tsv_error: '' });
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
				reason: gap.reason,
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

	const scanned = buckets.accept_parity + buckets.reject_parity +
		buckets.over_acceptance.length +
		buckets.gap_known.length + buckets.gap_unexpected.length + buckets.gap_beyond_acorn.length;

	// The checkout exists (guarded above), so an empty scan means a wrong subtree
	// path or a gutted corpus — a broken invocation, not a pass.
	if (scanned === 0) {
		console.error(`FAIL: 0 single-file .ts scanned under ${root} — wrong path? Nothing was graded.`);
		Deno.exit(1);
	}

	console.error(`\nTypeScript-repo parse-conformance triage — root: ${root}`);
	console.error(`  oracle: tsc baselines (${BASELINE_DIR})`);
	console.error(
		`  scanned: ${scanned} single-file .ts  (skipped ${skipped.multi_file} @filename, ${skipped.tsx} .tsx, ${skipped.unreadable} unreadable)\n`,
	);
	console.error(`  parity accept (tsv ok, tsc valid):     ${buckets.accept_parity}`);
	console.error(
		`  parity reject (tsv + tsc both reject): ${buckets.reject_parity}  (incl. acorn-over-leniency — no sanction needed)`,
	);
	console.error(
		`  over-acceptance (tsv ok, tsc invalid): ${buckets.over_acceptance.length}  (deferred early-errors; not gated)`,
	);
	console.error(
		`  GAPS known (tsc+acorn valid, tsv rejects): ${buckets.gap_known.length}  (${
			[...gap_by_category.entries()].map(([c, n]) => `${c}=${n}`).join(', ') || 'none'
		})`,
	);
	console.error(
		`  gap-beyond-acorn (tsc valid, acorn+tsv reject): ${buckets.gap_beyond_acorn.length}  (MIXED: acorn-ts gaps + early-error timing; manual triage; not gated)`,
	);
	console.error(
		`  GAPS UNEXPECTED (untracked): ${buckets.gap_unexpected.length}  (GATES)`,
	);

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
			skipped,
			accept_parity: buckets.accept_parity,
			reject_parity: buckets.reject_parity,
			over_acceptance: buckets.over_acceptance,
			gap_known: buckets.gap_known,
			gap_known_by_category: Object.fromEntries(gap_by_category),
			gap_beyond_acorn: buckets.gap_beyond_acorn,
			gap_unexpected: buckets.gap_unexpected,
		};
		Deno.stdout.writeSync(new TextEncoder().encode(JSON.stringify(report, null, '\t') + '\n'));
	}

	if (buckets.gap_unexpected.length > 0) {
		console.error(
			`\nFAIL: ${buckets.gap_unexpected.length} untracked gap(s) — tsv rejects input BOTH tsc and acorn accept. ` +
				`Fix the parser, or add a reasoned KNOWN_GAPS entry (this file, separate from the acorn-suite gate).`,
		);
		Deno.exit(1);
	}

	// Full-corpus-only hygiene (a subtree run legitimately grades a slice):
	if (root === DEFAULT_ROOT) {
		// Ledger freshness: a KNOWN_GAPS entry matching nothing means its gap was
		// fixed (delete it) or upstream renamed the test (update it) — the list must
		// mirror the live corpus, like scan_audit's ALLOW list.
		const stale = KNOWN_GAPS.filter((g) => !used_gaps.has(g.pattern));
		if (stale.length > 0) {
			console.error(
				`\nFAIL: ${stale.length} stale KNOWN_GAPS entr${stale.length === 1 ? 'y' : 'ies'} — matched no over-rejection:\n` +
					stale.map((g) => `    · ${g.pattern}`).join('\n'),
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
		].filter((f): f is string => f !== null);
		if (pin_failures.length > 0) {
			console.error(
				`\nFAIL: pinned count mismatch — ${pin_failures.join('; ')}. If this move is deliberate ` +
					`(checkout pull, behavior change), re-pin in lib/gate_counts.ts (see its update ritual).`,
			);
			Deno.exit(1);
		}
	}

	console.error(
		`\nOK: no untracked gaps (all tsv over-rejections are tracked or acorn-confirmed-invalid).`,
	);
}

if (import.meta.main) {
	await run_ts_repo_compare();
}

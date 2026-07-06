/**
 * TypeScript-repo parse-conformance triage — runs tsv's TS parser against the
 * official `microsoft/typescript` compiler's own parser test corpus
 * (`../typescript/tests/cases/conformance/parser`, ~800 single-file `.ts`), using
 * **tsc's own baselines as the validity oracle** (L1) instead of a live parser.
 *
 * Why tsc baselines, not acorn: acorn-typescript is tsv's drop-in *target* but is
 * itself over-lenient (it accepts invalid TS the real compiler rejects), so it's
 * an imperfect *validity* oracle. tsc is authoritative. The TS test harness writes
 * `tests/baselines/reference/<name>.errors.txt` iff a compile produces diagnostics;
 * a `TS1xxx` code there is a **syntax/grammar** error (tsc's parser rejects), while
 * `TS2xxx`+ are semantic (tsc's parser accepted). So:
 *
 *   tsc-syntax-valid(file)  ⇔  no `<name>.errors.txt`, or none of its codes is TS1xxx.
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
 * Standalone triage tool (like `skip_triage.ts`), NOT in the blocking `conformance`
 * aggregate — the corpus is large and grows; promote to a gate once curated. Exits
 * 1 on an un-tracked `gap` so it CAN gate. Summary to stderr, full JSON to stdout.
 *
 * Run (from the repo root):
 *   deno task conformance:ts-repo            # builds corpus FFI, then runs
 *   deno task conformance:ts-repo:run        # skip rebuild (freshness-guarded)
 *   deno task conformance:ts-repo:run --json 2>/dev/null > report.json
 *   deno task conformance:ts-repo:run ../typescript/tests/cases/conformance/parser/ecmascript6
 */

import { readdir, readFile } from 'node:fs/promises';
import { basename, join } from 'node:path';

import { init_compare_implementations } from '../lib/compare_cli.ts';
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
 * grimoire TODO_PARSE_COVERAGE.md §"Broadening — the official typescript repo".
 * `pattern` uses `<basename>.ts` to avoid numeric-suffix collisions
 * (`…Declaration1` vs `…Declaration11`).
 */
const KNOWN_GAPS: KnownGap[] = [
	// Gap B — consecutive computed class members across ASI (no `;` between them).
	{ pattern: 'parserComputedPropertyName29.ts', category: 'computed-member-asi', reason: 'class { [e] = id++⏎[e2]: number }' },
	{ pattern: 'parserComputedPropertyName31.ts', category: 'computed-member-asi', reason: 'class { [e]: number⏎[e2]: number }' },
	// Gap C — accessor WITH A BODY in a `declare` class (parse_declare_class_member).
	{ pattern: 'parserAccessors5.ts', category: 'declare-accessor-body', reason: 'declare class { get foo() { return 0 } }' },
	{ pattern: 'parserAccessors6.ts', category: 'declare-accessor-body', reason: 'declare class { set foo(v) {} }' },
];

const flags = new Set(Deno.args.filter((a) => a.startsWith('-')));
const json_mode = flags.has('--json');
const verbose = flags.has('--verbose') || flags.has('-v');
const root = Deno.args.find((a) => !a.startsWith('-')) ?? DEFAULT_ROOT;

/** tsc's parser verdict for a test, read from its baseline. `null` = baseline unreadable. */
async function tsc_syntax_valid(file_path: string): Promise<boolean> {
	const base = basename(file_path).replace(/\.ts$/, '');
	try {
		const errors = await readFile(join(BASELINE_DIR, `${base}.errors.txt`), 'utf8');
		// A TS1xxx code = a syntax/grammar diagnostic → tsc's parser rejects.
		return !/error TS1\d{3}:/.test(errors);
	} catch {
		// No baseline → the compile was clean → tsc accepts.
		return true;
	}
}

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

const { canonical, native } = await init_compare_implementations();

interface Gap {
	path: string;
	tsv_error: string;
	category?: string;
	reason?: string;
}
const buckets = {
	accept_parity: 0,
	reject_parity: 0,
	over_acceptance: [] as { path: string; tsv_error: string }[],
	gap_known: [] as Gap[],
	gap_unexpected: [] as Gap[],
	gap_beyond_acorn: [] as { path: string; tsv_error: string }[],
};
const skipped = { tsx: 0, multi_file: 0, unreadable: 0 };

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
		buckets.gap_known.push({ path, tsv_error: tsv_err, category: gap.category, reason: gap.reason });
	} else {
		buckets.gap_unexpected.push({ path, tsv_error: tsv_err });
	}
}

canonical.dispose();
native.dispose();

// --- Report -------------------------------------------------------------------

const gap_by_category = new Map<string, number>();
for (const g of buckets.gap_known) {
	gap_by_category.set(g.category!, (gap_by_category.get(g.category!) ?? 0) + 1);
}

const scanned = buckets.accept_parity + buckets.reject_parity + buckets.over_acceptance.length +
	buckets.gap_known.length + buckets.gap_unexpected.length + buckets.gap_beyond_acorn.length;

console.error(`\nTypeScript-repo parse-conformance triage — root: ${root}`);
console.error(`  oracle: tsc baselines (${BASELINE_DIR})`);
console.error(
	`  scanned: ${scanned} single-file .ts  (skipped ${skipped.multi_file} @filename, ${skipped.tsx} .tsx, ${skipped.unreadable} unreadable)\n`,
);
console.error(`  parity accept (tsv ok, tsc valid):     ${buckets.accept_parity}`);
console.error(`  parity reject (tsv + tsc both reject): ${buckets.reject_parity}  (incl. acorn-over-leniency — no sanction needed)`);
console.error(`  over-acceptance (tsv ok, tsc invalid): ${buckets.over_acceptance.length}  (deferred early-errors; not gated)`);
console.error(
	`  GAPS known (tsc+acorn valid, tsv rejects): ${buckets.gap_known.length}  (${
		[...gap_by_category.entries()].map(([c, n]) => `${c}=${n}`).join(', ') || 'none'
	})`,
);
console.error(`  gap-beyond-acorn (tsc valid, acorn+tsv reject): ${buckets.gap_beyond_acorn.length}  (MIXED: acorn-ts gaps + early-error timing; manual triage; not gated)`);
console.error(`  GAPS UNEXPECTED (untracked): ${buckets.gap_unexpected.length}  (GATES)`);

for (const g of buckets.gap_unexpected) {
	console.error(`\n    ✗ UNEXPECTED gap: ${g.path}\n        ${g.tsv_error}`);
}
if (verbose) {
	for (const g of buckets.gap_known) console.error(`      · known-gap [${g.category}] ${basename(g.path)} — ${g.reason}`);
	for (const g of buckets.gap_beyond_acorn) console.error(`      · beyond-acorn ${basename(g.path)} — ${g.tsv_error}`);
	for (const g of buckets.over_acceptance) console.error(`      · over-acceptance ${basename(g.path)}`);
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
console.error(`\nOK: no untracked gaps (all tsv over-rejections are tracked or acorn-confirmed-invalid).`);

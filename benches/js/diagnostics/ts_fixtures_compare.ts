/**
 * TypeScript-fixtures parse-conformance gate — the drop-in-parser analog of
 * test262 (JS) and the Svelte-fixtures harness, run against acorn-typescript's
 * OWN test suite (`../acorn-typescript/test`, ~200 adversarial `input.ts`
 * fixtures: arrow-type params, class/decorator edge cases, import attributes,
 * escaped keywords, …). tsv is a drop-in replacement for acorn + acorn-typescript,
 * so its own regression corpus is the natural TS edge-case oracle — the piece the
 * real-world corpus (`corpus:compare:parse`) can't reach, because ordinary code
 * doesn't exercise these shapes.
 *
 * Oracle = the LIVE `@sveltejs/acorn-typescript` parser (pinned in
 * benches/js/package.json / sidecar.ts), NOT the committed `expected.json`
 * artifacts. Why: the same reason the Svelte gate uses the live modern parser —
 * a committed artifact can drift from the pinned version that defines fixture
 * correctness, and the live parser is exactly what `corpus:compare:parse` already
 * diffs against, so the two stay consistent by construction.
 *
 * Two comparisons in one pass over each `input.ts`:
 *
 *   (1) VERDICT parity — does tsv accept/reject exactly what acorn-typescript
 *       does? Bucketed by *asymmetry*, not raw error count:
 *         - `parity`            — both reject (an intentional-error fixture, or a
 *                                 shape both decline, e.g. JSX in a `.ts` file);
 *                                 the healthy state.
 *         - `over_acceptance`   — tsv accepts, acorn rejects; a deferred
 *                                 early-error (tsv's documented posture). Reported,
 *                                 never gates.
 *         - over-rejection (tsv rejects, acorn accepts), split three ways:
 *             · `sanctioned` — tsv over-rejects *deliberately* (deprecated syntax
 *               it declines, or input its own grammar rejects). See
 *               lib/parse_sanctions.ts TS_FIXTURE_SANCTIONS.
 *             · `known_gap`  — tsv is *wrong*; a tracked drop-in gap (KNOWN_GAPS
 *               below). Reported, does not gate; the set should only SHRINK.
 *             · `unexpected` — an over-rejection in neither list: a NEW gap. GATES.
 *
 *   (2) AST-shape — for inputs both accept, deep-diff tsv's wire AST against the
 *       acorn AST via the corpus_compare_parse.ts engine (`diff_asts` +
 *       `DOCUMENTED_MATCHERS`). Undocumented diff groups are a REPORT-ONLY triage
 *       surface (not gating), mirroring the Svelte gate: an undocumented group is
 *       either a real writer/parser bug to fix or a divergence to catalogue into
 *       the SHARED `DOCUMENTED_MATCHERS` (which also shrinks the
 *       `corpus:compare:parse` count). Unlike the Svelte tree's large backlog,
 *       this corpus is near-clean, so promoting AST-shape to a gate once the
 *       count hits 0 is a natural near-term follow-up.
 *
 * Scope: every `input.ts` under the suite root (the `*.test.ts` / `utils.ts`
 * harness files are excluded by basename). `.tsx`/JSX fixtures parse as ordinary
 * `.ts` here — tsv and acorn (module mode, no JSX plugin) both reject them, so
 * they land in `parity`.
 *
 * Periodic (non-`check`) gate — needs the FFI + the acorn-typescript oracle
 * (node_modules) + the `../acorn-typescript/test` checkout, so it can't run on a
 * clean checkout. Fail-open on a missing root (0 scanned → green), matching the
 * publish gate's tolerance for absent oracles. Full JSON to stdout, summary to
 * stderr. Exit 1 on any `unexpected` over-rejection (the enforced verdict gate);
 * the AST-shape count is reported but does not gate.
 *
 * Run (from the repo root):
 *   deno task conformance:ts-fixtures                # builds corpus FFI, then runs
 *   deno task conformance:ts-fixtures:run            # skip rebuild (freshness-guarded)
 *   deno task conformance:ts-fixtures:run --json 2>/dev/null > report.json
 *   deno task conformance:ts-fixtures:run ../acorn-typescript/test/class_accessor
 */

import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { init_compare_implementations } from '../lib/compare_cli.ts';
import { sanction_for, TS_FIXTURE_SANCTIONS } from '../lib/parse_sanctions.ts';
import { bigint_replacer, type DiffEntry, diff_asts, type MatchContext } from '../corpus_compare_parse.ts';

const DEFAULT_ROOT = '../acorn-typescript/test';

/** Canonical TS parse-INPUT basename (excludes `*.test.ts`, `utils.ts`, `run_test262.js`). */
const INPUT_BASENAMES = new Set(['input.ts']);

/** Directory names pruned wholesale during discovery. */
function prune_dir(name: string): boolean {
	return name.startsWith('_') || name === 'node_modules';
}

// Over-rejections tsv keeps deliberately — shared with the general parity gate,
// one home in lib/parse_sanctions.ts. A genuine gap goes in KNOWN_GAPS, never here.
const SANCTIONED = TS_FIXTURE_SANCTIONS;

/**
 * Over-rejections where tsv is WRONG — genuine drop-in parse gaps, tracked so the
 * gate is green at baseline and only regressions (a NEW, untracked over-rejection)
 * fail it. This set must only SHRINK: when a gap is fixed, delete its entry (the
 * input then parses → parity). Full triage lives in the grimoire lore
 * (TODO_CORPUS_FORMATTING.md §Parser-gap bucket).
 */
const KNOWN_GAPS: { pattern: string; category: string; reason: string }[] = [
	{
		pattern: 'normal_syntax_import_type_specifier_with_as/',
		category: 'import-type-specifier',
		reason:
			"`import { type as age }` — `type` is the imported name, `as age` the rename; tsv reads " +
			'`type` as the type-modifier keyword and over-rejects. (`import { type as as x }` already parses.)',
	},
];

interface OverRejection {
	path: string;
	error: string;
}

async function* discover(root: string): AsyncGenerator<{ path: string; content: string }> {
	let entries;
	try {
		entries = await readdir(root, { withFileTypes: true });
	} catch (e) {
		console.error(`Cannot read ${root}: ${e instanceof Error ? e.message : e}`);
		return;
	}
	for (const entry of entries) {
		const full = join(root, entry.name);
		if (entry.isDirectory()) {
			if (!prune_dir(entry.name)) yield* discover(full);
		} else if (INPUT_BASENAMES.has(entry.name)) {
			yield { path: full, content: await readFile(full, 'utf8') };
		}
	}
}

function first_line(e: unknown): string {
	return String(e instanceof Error ? e.message : e).split('\n')[0];
}

const args = new Set(Deno.args.filter((a) => a.startsWith('-')));
const json_mode = args.has('--json');
const verbose = args.has('--verbose') || args.has('-v');
const root = Deno.args.find((a) => !a.startsWith('-')) ?? DEFAULT_ROOT;

const { canonical, native } = await init_compare_implementations();

const buckets = {
	parity: 0,
	both_accept: 0,
	over_acceptance: [] as OverRejection[],
	sanctioned: [] as (OverRejection & { reason: string })[],
	known_gap: [] as (OverRejection & { category: string; reason: string })[],
	unexpected: [] as OverRejection[],
};

/** AST-diff groups keyed by signature, split documented vs undocumented. */
interface AstGroup {
	signature: string;
	documented: string | null;
	files: Set<string>;
	count: number;
	sample: { path: string; entry: DiffEntry } | null;
}
const ast_groups = new Map<string, AstGroup>();
let ast_clean = 0;

function record_ast(path: string, diffs: DiffEntry[]): void {
	if (diffs.length === 0) {
		ast_clean++;
		return;
	}
	for (const entry of diffs) {
		const key = `${entry.signature}\0${entry.documented ?? ''}`;
		let g = ast_groups.get(key);
		if (!g) {
			g = { signature: entry.signature, documented: entry.documented, files: new Set(), count: 0, sample: null };
			ast_groups.set(key, g);
		}
		g.files.add(path);
		g.count++;
		if (!g.sample) g.sample = { path, entry };
	}
}

let scanned = 0;
for await (const file of discover(root)) {
	scanned++;

	let tsv_ast: unknown;
	let tsv_err: string | null = null;
	try {
		tsv_ast = native.parse(file.content, 'typescript');
	} catch (e) {
		tsv_err = first_line(e);
	}

	let canon_ast: unknown;
	let canon_err: string | null = null;
	try {
		canon_ast = canonical.parse(file.content, 'typescript');
	} catch (e) {
		canon_err = first_line(e);
	}

	if (tsv_err && canon_err) {
		buckets.parity++;
	} else if (tsv_err && !canon_err) {
		// Over-rejection — classify sanctioned / known-gap / unexpected.
		const reason = sanction_for(SANCTIONED, file.path);
		const gap = KNOWN_GAPS.find((g) => file.path.includes(g.pattern));
		if (reason) {
			buckets.sanctioned.push({ path: file.path, error: tsv_err, reason });
		} else if (gap) {
			buckets.known_gap.push({ path: file.path, error: tsv_err, category: gap.category, reason: gap.reason });
		} else {
			buckets.unexpected.push({ path: file.path, error: tsv_err });
		}
	} else if (!tsv_err && canon_err) {
		buckets.over_acceptance.push({ path: file.path, error: canon_err });
	} else {
		// Both accept — deep-diff the ASTs (canonical serialized like the sidecar).
		buckets.both_accept++;
		const canonical_root = JSON.parse(JSON.stringify(canon_ast, bigint_replacer));
		const ctx: MatchContext = { source: file.content, canonical_root };
		const { diffs } = diff_asts(tsv_ast, canonical_root, ctx);
		record_ast(file.path, diffs);
	}
}

canonical.dispose();
native.dispose();

// --- Report -------------------------------------------------------------------

const undocumented_groups = [...ast_groups.values()].filter((g) => g.documented === null);
const documented_groups = [...ast_groups.values()].filter((g) => g.documented !== null);

const gap_by_category = new Map<string, number>();
for (const g of buckets.known_gap) {
	gap_by_category.set(g.category, (gap_by_category.get(g.category) ?? 0) + 1);
}

console.error(`\nTypeScript-fixtures parse-conformance gate — root: ${root}`);
console.error(`  scanned: ${scanned} .ts inputs\n`);
console.error(`  VERDICT`);
console.error(`    parity (both reject):     ${buckets.parity}`);
console.error(`    both accept:              ${buckets.both_accept}`);
console.error(`    over-acceptance:          ${buckets.over_acceptance.length}  (deferred early-errors; not gated)`);
console.error(`    over-rejection sanctioned:${buckets.sanctioned.length}  (deliberate; see TS_FIXTURE_SANCTIONS)`);
console.error(
	`    over-rejection known-gap: ${buckets.known_gap.length}  (tracked; ${
		[...gap_by_category.entries()].map(([c, n]) => `${c}=${n}`).join(', ') || 'none'
	})`,
);
console.error(`    over-rejection UNEXPECTED: ${buckets.unexpected.length}  (new gap — GATES)`);
console.error(`\n  AST-SHAPE (of the ${buckets.both_accept} both-accept)`);
console.error(`    clean:                    ${ast_clean}`);
console.error(`    documented diff groups:   ${documented_groups.length}`);
console.error(`    undocumented diff groups: ${undocumented_groups.length}  (report-only — triage surface)`);

for (const e of buckets.unexpected) {
	console.error(`\n    ✗ UNEXPECTED over-rejection: ${e.path}\n        ${e.error}`);
}
// AST-shape groups are a report-only triage surface — full detail only with -v
// (or the --json report), so a green verdict run isn't buried under the groups.
if (verbose) {
	for (const g of undocumented_groups) {
		console.error(
			`      · undocumented AST group: ${g.signature}  (${g.count} in ${g.files.size} file(s))` +
				(g.sample ? `  e.g. ${g.sample.path}` : ''),
		);
	}
	for (const g of buckets.known_gap) console.error(`      · known-gap [${g.category}] ${g.path}`);
	for (const e of buckets.over_acceptance) console.error(`      · over-acceptance ${e.path}`);
}

const report = {
	root,
	scanned,
	verdict: {
		parity: buckets.parity,
		both_accept: buckets.both_accept,
		over_acceptance: buckets.over_acceptance,
		sanctioned: buckets.sanctioned,
		known_gap: buckets.known_gap,
		known_gap_by_category: Object.fromEntries(gap_by_category),
		unexpected: buckets.unexpected,
	},
	ast: {
		both_accept: buckets.both_accept,
		clean: ast_clean,
		documented_groups: documented_groups.map((g) => ({
			signature: g.signature,
			matcher: g.documented,
			count: g.count,
			files: g.files.size,
		})),
		undocumented_groups: undocumented_groups.map((g) => ({
			signature: g.signature,
			count: g.count,
			files: [...g.files].slice(0, 10),
			sample: g.sample ? { path: g.sample.path, entry: g.sample.entry } : null,
		})),
	},
};

if (json_mode) {
	Deno.stdout.writeSync(new TextEncoder().encode(JSON.stringify(report, null, '\t') + '\n'));
}

if (undocumented_groups.length > 0) {
	console.error(
		`\nNOTE: ${undocumented_groups.length} undocumented AST-shape group(s) to triage (report-only, not ` +
			`gating) — catalog each into corpus_compare_parse.ts DOCUMENTED_MATCHERS (shared; also shrinks ` +
			`the corpus:compare:parse count) or fix as a writer/parser bug. Detail: -v or --json.`,
	);
}
// Only the verdict half enforces: a NEW over-rejection in neither SANCTIONED nor
// KNOWN_GAPS is a genuine drop-in regression.
if (buckets.unexpected.length > 0) {
	console.error(
		`\nFAIL: ${buckets.unexpected.length} unexpected over-rejection(s) — tsv rejects input acorn-typescript ` +
			`accepts, and it's neither sanctioned nor a tracked gap. Fix the parser, or — if tsv is correctly ` +
			`stricter — add a reasoned TS_FIXTURE_SANCTIONS entry; if it's a known gap, add a KNOWN_GAPS entry ` +
			`(this file).`,
	);
	Deno.exit(1);
}
console.error(`\nOK: verdict parity holds (no unexpected over-rejections).`);

/**
 * Svelte-fixtures parse-conformance gate — the drop-in-parser analog of test262
 * (JS) and the WPT harness (CSS), run against Svelte's own compiler test suite
 * (`../svelte/packages/svelte/tests`).
 *
 * Oracle = the LIVE modern Svelte parser (`svelte/compiler` `parse(src,
 * {modern:true})`), NOT the committed fixture artifacts. Why: `output.json` in
 * `parser-legacy` is the *legacy* AST (tsv targets the modern parser);
 * `compiler-errors/_config.js` encodes *compiler* verdicts (often analysis-stage,
 * post-parse), not parse-stage ones; `css` ships compiled CSS. The modern parser
 * is the only correct oracle for a drop-in *modern-parser* replacement — and it
 * makes the two "trap" partitions resolve for free: `loose-*` inputs throw under
 * the non-loose oracle (→ parity), and analysis-stage `compiler-errors` parse
 * fine on both sides (→ never miscounted as a tsv bug).
 *
 * Two comparisons in one pass over each `.svelte` input:
 *
 *   (1) VERDICT parity — does tsv accept/reject exactly what the Svelte parser
 *       does? Bucketed by *asymmetry*, not raw error count:
 *         - `parity`            — both reject (an intentional-error fixture); healthy.
 *         - `over_acceptance`   — tsv accepts, Svelte rejects; a deferred early-error
 *                                 (tsv's documented posture). Reported, never gates.
 *         - over-rejection (tsv rejects, Svelte accepts), split three ways:
 *             · `sanctioned` — tsv is *correctly* stricter (input is invalid Svelte
 *               the parser is merely lenient about; its own validator rejects).
 *             · `known_gap`  — tsv is *wrong*; a tracked drop-in gap (see
 *               ../../grimoire lore TODO_PARSE_COVERAGE). Reported, does not gate;
 *               the set should only shrink.
 *             · `unexpected` — an over-rejection in neither list: a NEW gap. GATES.
 *
 *   (2) AST-shape — for inputs both accept, deep-diff tsv's wire AST against the
 *       Svelte AST via the corpus_compare_parse.ts engine (`diff_asts` +
 *       `DOCUMENTED_MATCHERS`). Undocumented diff groups are surfaced as a
 *       REPORT-ONLY triage surface (not yet gating): the adversarial fixture tree
 *       exposes many edge divergences that must be triaged into the SHARED
 *       `DOCUMENTED_MATCHERS` (which then shrinks this count for free) or fixed as
 *       writer bugs. Enforcing it before that campaign would be red-forever noise.
 *
 * Scope: the canonical `.svelte` INPUTS across `tests/` (`input.svelte` /
 * `main.svelte` / `index.svelte`), skipping generated `_`-prefixed artifacts,
 * `output.svelte` dups, and the `migrate/` tree (Svelte-4 migrator inputs, not
 * modern-parse targets). `.svelte.js`/`.ts`/`.css` are out of scope here — the
 * TS/CSS parsers have their own corpora (test262, wpt).
 *
 * Periodic (non-`check`) gate — needs the FFI + the `svelte/compiler` oracle,
 * so it can't run on a clean checkout. Full JSON to stdout, summary to stderr.
 * Exit 1 on any `unexpected` over-rejection (the enforced verdict gate); the
 * AST-shape count is reported but does not gate (see above).
 *
 * Run (from the repo root):
 *   deno task conformance:svelte-fixtures             # builds corpus FFI, then runs
 *   deno task conformance:svelte-fixtures:run         # skip rebuild (freshness-guarded)
 *   deno task conformance:svelte-fixtures:run --json 2>/dev/null > report.json
 *   deno task conformance:svelte-fixtures:run ../svelte/packages/svelte/tests/parser-modern
 */

import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { init_compare_implementations } from '../lib/compare_cli.ts';
import { sanction_for, SVELTE_FIXTURE_SANCTIONS } from '../lib/parse_sanctions.ts';
import { bigint_replacer, type DiffEntry, diff_asts, type MatchContext } from '../corpus_compare_parse.ts';

const DEFAULT_ROOT = '../svelte/packages/svelte/tests';

/** Canonical Svelte parse-INPUT basenames (excludes `output.svelte`, `*.svelte.js`). */
const INPUT_BASENAMES = new Set(['input.svelte', 'main.svelte', 'index.svelte']);

/** Directory names pruned wholesale during discovery. */
function prune_dir(name: string): boolean {
	return (
		name.startsWith('_') || // generated artifacts (_output/_expected/_actual)
		name === 'node_modules' ||
		name === '.svelte-kit' ||
		// Svelte-4 → 5 migrator INPUTS: intentionally legacy/weird, not modern-parse
		// targets. Out of scope for a modern-parser conformance gate.
		name === 'migrate'
	);
}

// Over-rejections where tsv is *correctly* stricter (the input is invalid Svelte
// the parser is merely lenient about) — shared with skip_triage.ts, one home in
// lib/parse_sanctions.ts. A genuine gap goes in KNOWN_GAPS (below), never here.
const SANCTIONED = SVELTE_FIXTURE_SANCTIONS;

/**
 * Over-rejections where tsv is WRONG — genuine drop-in parse gaps, tracked so the
 * gate is green at baseline and only regressions (a NEW, untracked over-rejection)
 * fail it. This set must only SHRINK: when a gap is fixed, delete its entry (the
 * input then parses → parity). Full triage lives in the grimoire lore
 * (TODO_PARSE_COVERAGE.md §"Svelte parse over-rejections vs `svelte/tests`").
 */
const KNOWN_GAPS: { pattern: string; category: string; reason: string }[] = [
	// `<textarea>` RCDATA — its content is raw text with live `{expr}` interpolation
	// (the inner `<p>` is text, not an element) read up to a whitespace-tolerant
	// `</textarea…>`. A sibling of the `<script>`/`<style>` raw-text path, tracked
	// separately from the (now-closed) implicit-tag-closing cluster; deferred.
	{
		pattern: 'parser-legacy/samples/textarea-end-tag/',
		category: 'textarea-rcdata',
		reason: '`<textarea>` raw-text content + `{expr}` + whitespace-tolerant close',
	},
	// Attribute-name lexer over-strict on characters HTML permits in attr names.
	{
		pattern: 'runtime-runes/samples/props-alias-weird/',
		category: 'attr-name-lexer',
		reason: "`%` in an attribute name (`ysc%%gibberish`) — permitted by HTML; tsv's attr-name lexer rejects it",
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
		tsv_ast = native.parse(file.content, 'svelte');
	} catch (e) {
		tsv_err = first_line(e);
	}

	let canon_ast: unknown;
	let canon_err: string | null = null;
	try {
		canon_ast = canonical.parse(file.content, 'svelte');
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

console.error(`\nSvelte-fixtures parse-conformance gate — root: ${root}`);
console.error(`  scanned: ${scanned} .svelte inputs\n`);
console.error(`  VERDICT`);
console.error(`    parity (both reject):     ${buckets.parity}`);
console.error(`    both accept:              ${buckets.both_accept}`);
console.error(`    over-acceptance:          ${buckets.over_acceptance.length}  (deferred early-errors; not gated)`);
console.error(`    over-rejection sanctioned:${buckets.sanctioned.length}  (tsv correctly stricter)`);
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
// (or the --json report), so a green verdict run isn't buried under 40 groups.
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
			`the corpus:compare:parse count) or fix as a writer bug. Detail: -v or --json.`,
	);
}
// Only the verdict half enforces: a NEW over-rejection in neither SANCTIONED nor
// KNOWN_GAPS is a genuine drop-in regression.
if (buckets.unexpected.length > 0) {
	console.error(
		`\nFAIL: ${buckets.unexpected.length} unexpected over-rejection(s) — tsv rejects input the modern ` +
			`Svelte parser accepts, and it's neither sanctioned nor a tracked gap. Fix the parser, or — if ` +
			`tsv is correctly stricter — add a reasoned SANCTIONED entry; if it's a known gap, add a ` +
			`KNOWN_GAPS entry (this file).`,
	);
	Deno.exit(1);
}
console.error(`\nOK: verdict parity holds (no unexpected over-rejections).`);

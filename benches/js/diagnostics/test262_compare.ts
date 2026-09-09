/**
 * Diagnostic: differential test262 conformance — tsv vs oxc-parser.
 *
 * Consumes the manifest emitted by
 *   cargo run -p tsv_debug test262 --emit-manifest <file>
 * (tsv's graded subset, one row per test — each row carries the test's
 * `expected` verdict and tsv's actual verdict), runs oxc-parser over the same
 * files at the same goal tsv grades each at (`module`-flagged → module, else
 * script) and under the same harness strict-mode transform (`graded_source`,
 * applied on the rows the manifest marks `strict`), and buckets the agreement so
 * a tsv failure can be triaged as a real bug vs. a shared limitation. A
 * mode-unflagged test is graded twice by the Rust runner but carries one row
 * here — its sloppy run — so both parsers see exactly one real run of it. The
 * two starred buckets are the actionable output:
 *   - positives where tsv rejects but oxc accepts → tsv real-bug candidates
 *   - negatives where oxc rejects but tsv accepts → tsv early-error gaps
 *
 * Not wired into `deno task` — run ad hoc from the repo root (full JSON to
 * stdout, summary to stderr). Needs the same permission set the bench uses for
 * oxc native:
 *   cargo run -p tsv_debug test262 --emit-manifest /tmp/t262.json
 *   deno run --allow-read --allow-env --allow-ffi --allow-net --allow-sys \
 *     --config benches/js/deno.json \
 *     benches/js/diagnostics/test262_compare.ts --manifest /tmp/t262.json \
 *     2>/dev/null > /tmp/t262-compare.json
 *
 * Background + bucket taxonomy: docs/conformance_test262.md §Differential.
 */

import * as oxc from 'oxc-parser';

type Verdict = 'accept' | 'reject';

interface ManifestEntry {
	relative_path: string;
	module: boolean;
	strict: boolean;
	expected: Verdict;
	tsv: Verdict;
}

interface Manifest {
	test262_root: string;
	count: number;
	tests: ManifestEntry[];
}

/** A single test's outcome across the two parsers, kept for the JSON buckets. */
interface Row {
	path: string;
	module: boolean;
	expected: Verdict;
	tsv: Verdict;
	oxc: Verdict;
}

/** Minimal flag reader (no flag-parsing dependency). */
function flag(name: string, fallback: string): string {
	const i = Deno.args.indexOf(`--${name}`);
	return i >= 0 && Deno.args[i + 1] ? Deno.args[i + 1] : fallback;
}

const manifest_path = flag('manifest', '/tmp/tsv-test262-manifest.json');
const examples = Number(flag('examples', '15'));

const manifest: Manifest = JSON.parse(await Deno.readTextFile(manifest_path));
console.error(
	`Loaded ${manifest.count} graded tests from ${manifest_path} (root ${manifest.test262_root})`
);

/**
 * The directive test262-harness inserts as the initial character sequence of a
 * test's source before running it in strict mode (test262/INTERPRETING.md
 * §Strict Mode). The Rust runner prepends the same string, so the differential
 * hands oxc exactly the source tsv parsed.
 */
const USE_STRICT_PREFIX = '"use strict";\n';

/**
 * The source to parse for one manifest row — the file's content with the
 * harness's strict-mode transform applied where the runner applies it.
 *
 * The manifest reports `strict` (the test is graded in strict mode by its own
 * declaration) and `module` (strict by the goal itself), so a row that is strict
 * at script goal is exactly an `onlyStrict` test, and that is the one the
 * harness prepends the directive to.
 */
function graded_source(source: string, entry: ManifestEntry): string {
	return entry.strict && !entry.module ? USE_STRICT_PREFIX + source : source;
}

/**
 * oxc's accept/reject verdict for one source, parsed at the test's goal to
 * mirror tsv: `module`-flagged tests as a module, everything else (the
 * run-both-ways default, `onlyStrict`, `noStrict` and `raw`) as a script (`module`
 * comes from the manifest). So an `await`-as-identifier test, valid only in a script, lands in
 * `both-accept` rather than `both-reject`.
 *
 * The goal axis matches, and strictness is not carried by the goal on either
 * side: oxc's `'script'` is sloppy and so is tsv's `Goal::Script`. The source is
 * what makes a row strict on both: a row the manifest marks `strict` at script
 * goal is handed over with the harness's `"use strict";` prefix
 * (`graded_source`). What remains is a row that is neither `module` nor `strict`
 * — sloppy on both sides, but tsv refuses the Annex B grammar under every mode —
 * the residual caveat docs/conformance_test262.md §Differential describes.
 *
 * A non-empty `errors` array, or a throw, counts as reject.
 */
function oxc_verdict(filename: string, source: string, module: boolean): Verdict {
	try {
		const result = oxc.parseSync(filename, source, {
			sourceType: module ? 'module' : 'script'
		});
		return result.errors && result.errors.length > 0 ? 'reject' : 'accept';
	} catch {
		return 'reject';
	}
}

const buckets = {
	// positives (test262 expects accept)
	pos_both_accept: [] as Row[],
	pos_tsv_bug: [] as Row[], // tsv reject, oxc accept  ← actionable real-bug list
	pos_oxc_gap: [] as Row[], // tsv accept, oxc reject  (oxc gap / tsv broader)
	pos_both_reject: [] as Row[], // shared limitation / test artifact
	// negatives (test262 expects reject, parse phase)
	neg_both_reject: [] as Row[],
	neg_tsv_gap: [] as Row[], // tsv accept, oxc reject  ← actionable early-error gap
	neg_both_accept: [] as Row[], // neither enforces (oxc under-enforces too)
	neg_tsv_stricter: [] as Row[] // tsv reject, oxc accept (rare)
};

let read_errors = 0;
let processed = 0;

for (const t of manifest.tests) {
	processed++;
	if (processed % 5000 === 0) console.error(`  ${processed}/${manifest.count}…`);

	let source: string;
	try {
		source = await Deno.readTextFile(`${manifest.test262_root}/${t.relative_path}`);
	} catch {
		read_errors++;
		continue;
	}

	const oxc_v = oxc_verdict(t.relative_path, graded_source(source, t), t.module);
	const row: Row = {
		path: t.relative_path,
		module: t.module,
		expected: t.expected,
		tsv: t.tsv,
		oxc: oxc_v
	};

	if (t.expected === 'accept') {
		if (t.tsv === 'accept' && oxc_v === 'accept') buckets.pos_both_accept.push(row);
		else if (t.tsv === 'reject' && oxc_v === 'accept') buckets.pos_tsv_bug.push(row);
		else if (t.tsv === 'accept' && oxc_v === 'reject') buckets.pos_oxc_gap.push(row);
		else buckets.pos_both_reject.push(row);
	} else {
		if (t.tsv === 'reject' && oxc_v === 'reject') buckets.neg_both_reject.push(row);
		else if (t.tsv === 'accept' && oxc_v === 'reject') buckets.neg_tsv_gap.push(row);
		else if (t.tsv === 'accept' && oxc_v === 'accept') buckets.neg_both_accept.push(row);
		else buckets.neg_tsv_stricter.push(row);
	}
}

const graded = manifest.count - read_errors;
const positives =
	buckets.pos_both_accept.length +
	buckets.pos_tsv_bug.length +
	buckets.pos_oxc_gap.length +
	buckets.pos_both_reject.length;
const negatives =
	buckets.neg_both_reject.length +
	buckets.neg_tsv_gap.length +
	buckets.neg_both_accept.length +
	buckets.neg_tsv_stricter.length;

// Pass rate over the SAME graded subset — a like-for-like baseline for tsv's
// numbers (a parser "passes" a test iff its verdict matches `expected`).
const tsv_pass =
	buckets.pos_both_accept.length +
	buckets.pos_oxc_gap.length + // tsv accept == expected accept
	buckets.neg_both_reject.length +
	buckets.neg_tsv_stricter.length; // tsv reject == expected reject
const oxc_pass =
	buckets.pos_both_accept.length +
	buckets.pos_tsv_bug.length + // oxc accept == expected accept
	buckets.neg_both_reject.length +
	buckets.neg_tsv_gap.length; // oxc reject == expected reject

const pct = (n: number, d: number) => (d > 0 ? ((n / d) * 100).toFixed(1) : '0.0');

function summarize(label: string, rows: Row[], total: number, star = false): string {
	const mark = star ? ' ←' : '  ';
	return `  ${label.padEnd(26)} ${String(rows.length).padStart(6)}  (${pct(
		rows.length,
		total
	)}%)${mark}`;
}

console.error(`\ntest262 differential: tsv vs oxc-parser`);
console.error(
	`oxc parsed at each row's declared goal: module rows as module, the rest as script, ` +
		`rows the manifest marks strict behind the harness's "use strict" prefix`
);
console.error(`graded subset: ${graded} tests  (${positives} positive, ${negatives} negative)`);
if (read_errors) console.error(`(${read_errors} files unreadable, skipped)`);
console.error(
	`pass rate (same subset):  tsv ${tsv_pass}/${graded} (${pct(tsv_pass, graded)}%)   ` +
		`oxc ${oxc_pass}/${graded} (${pct(oxc_pass, graded)}%)`
);

console.error(`\npositives (expected accept): ${positives}`);
console.error(summarize('both accept', buckets.pos_both_accept, positives));
console.error(summarize('tsv rejects, oxc accepts', buckets.pos_tsv_bug, positives, true));
console.error(summarize('oxc rejects, tsv accepts', buckets.pos_oxc_gap, positives));
console.error(summarize('both reject', buckets.pos_both_reject, positives));

console.error(`\nnegatives (expected reject): ${negatives}`);
console.error(summarize('both reject', buckets.neg_both_reject, negatives));
console.error(summarize('oxc rejects, tsv accepts', buckets.neg_tsv_gap, negatives, true));
console.error(summarize('both accept', buckets.neg_both_accept, negatives));
console.error(summarize('tsv rejects, oxc accepts', buckets.neg_tsv_stricter, negatives));

function examples_of(label: string, rows: Row[]): void {
	if (!rows.length) return;
	console.error(`\n${label} (first ${Math.min(examples, rows.length)} of ${rows.length}):`);
	for (const r of rows.slice(0, examples)) console.error(`  ${r.path}`);
}
examples_of('★ tsv real-bug candidates (positive: tsv rejects, oxc accepts)', buckets.pos_tsv_bug);
examples_of('★ tsv early-error gaps (negative: oxc rejects, tsv accepts)', buckets.neg_tsv_gap);

// Full report to stdout. `pos_both_accept` is the bulk (both parsers correctly
// accept) with zero triage value — keep its count but omit its ~tens-of-thousands
// of paths so the JSON stays small, mirroring the corpus `--json` convention.
const { pos_both_accept: _bulk, ...triage_buckets } = buckets;
const report = {
	manifest: manifest_path,
	test262_root: manifest.test262_root,
	graded,
	read_errors,
	pass_rate: { tsv: tsv_pass, oxc: oxc_pass, total: graded },
	counts: Object.fromEntries(Object.entries(buckets).map(([k, v]) => [k, v.length])),
	buckets_note: 'pos_both_accept paths omitted (count only); all other buckets list full paths',
	buckets: triage_buckets
};
console.log(JSON.stringify(report, null, 2));

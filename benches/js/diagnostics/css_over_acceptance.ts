/**
 * Per-tool OVER-ACCEPTANCE over the conformance CSS corpus — the axis a coverage
 * number structurally cannot show, and the `parse/css` counterpart to
 * `ts_repo_over_acceptance.ts`.
 *
 * Parse coverage counts accepts, so it can only ever reward permissiveness. On
 * the TypeScript and Svelte surfaces the conformance corpora are filtered to
 * VALID inputs, which makes the number honest and leaves the opposite question
 * unanswered. CSS gets no such filter, and deliberately: the reference row is
 * `svelte/compiler`'s `parseCss`, which is **not a validity oracle in either
 * direction** — it accepts genuinely malformed CSS (`//` comments, a missing
 * semicolon) and rejects valid modern CSS it does not implement (`@supports
 * selector(…)`, `css-mixins` dashed functions). Filtering to what it accepts
 * would drop files tsv also fails and flatter tsv's own number. So the surface
 * keeps every file and this tool supplies the missing axis: over the files
 * `parseCss` rejects, how many does each tool accept?
 *
 * **Read it inverted: lower is better, and 0 is not the target.** tsv is a
 * drop-in for `parseCss`, so a tsv accept here is a divergence worth knowing
 * about; a postcss accept usually is not, because postcss does not validate
 * at-rule preludes or values at all and is answering a different question about
 * the same bytes. This is a PROFILE, not a gate.
 *
 * **The one pinned thing is the reject set's SIZE** ({@link CSS_REJECTS_PIN}),
 * and that is what earns the tool its keep. The set is deterministic given the
 * pins — prettier's checkout commit, the wpt-css harvest count, and the svelte
 * oracle version are all pinned elsewhere — so a move means one of them moved, or
 * the oracle's grammar did. Without the pin, a `parseCss` that started rejecting
 * (or accepting) wholesale would silently reshape the published `parse/css`
 * coverage row with nothing to catch it.
 *
 * `svelte/compiler` appears as a row and must read 0/N: it BUILT this list, so
 * any other number means the oracle disagrees with itself and the run fails
 * rather than publishing a profile graded against a moved reference.
 *
 * Run (from the repo root):
 *   deno task css:over-acceptance
 *   deno task css:over-acceptance --json 2>/dev/null > report.json
 *   deno task css:over-acceptance --verbose   # per-tool sample paths
 */

import { DevReposLoader } from '../lib/corpus.ts';
import { CSS_REJECTS_PIN } from '../lib/gate_counts.ts';
import { init_implementations } from '../lib/implementations.ts';
import type { TsvImplementation } from '../lib/types.ts';

/** Sample paths kept per tool for `--verbose` — enough to see the shape, not a dump. */
const SAMPLE_LIMIT = 8;

const json_mode = Deno.args.includes('--json');
const verbose = Deno.args.includes('--verbose') || Deno.args.includes('-v');
const log = (...args: unknown[]): void => {
	if (!json_mode) console.error(...args);
};

const impls = await init_implementations({ logger: log });

/** The CSS-parsing rows, in the conformance report's display order. */
const rows: Array<{ name: string; impl: TsvImplementation | undefined }> = [
	{ name: 'svelte/compiler', impl: impls.canonical },
	{ name: 'tsv', impl: impls.native },
	{ name: 'tsv_wasm', impl: impls.wasm },
	{ name: 'postcss', impl: impls.postcss }
];

const accepts = (impl: TsvImplementation, source: string): boolean => {
	try {
		impl.parse(source, 'css');
		return true;
	} catch {
		return false;
	}
};

// The conformance view is the surface this profile explains — prettier's CSS
// suite plus the wpt-css harvest, both pinned. The `gates`/`perf` views would
// fold in live dev repos, whose churn the pin cannot survive.
const files = (await new DevReposLoader('conformance').load(log)).filter(
	(f) => f.language === 'css'
);
if (files.length === 0) {
	console.error('FAIL: no CSS files in the conformance corpus — a partial checkout or harvest.');
	Deno.exit(1);
}

// Build the reject list with the oracle itself, live. Unlike the tsc corpus this
// needs no harvest cache: nothing else consumes the list (the CSS corpus is
// deliberately unfiltered), so a cache would be a second thing to keep fresh.
const oracle = impls.canonical;
const rejected = files.filter((f) => !accepts(oracle, f.content));

if (rejected.length !== CSS_REJECTS_PIN) {
	console.error(
		`FAIL: the oracle rejects ${rejected.length} of ${files.length} CSS files ≠ pinned ` +
			`${CSS_REJECTS_PIN}. Either a pinned input moved (../prettier's checkout, the wpt-css ` +
			`harvest) or svelte's parseCss changed what it accepts — re-pin CSS_REJECTS_PIN in ` +
			`lib/gate_counts.ts deliberately, after checking which.`
	);
	Deno.exit(1);
}

interface Row {
	name: string;
	accepted: number;
	total: number;
	rate: number;
	samples: string[];
}

const results: Row[] = [];
for (const { name, impl } of rows) {
	if (!impl?.supports_parse_language('css')) {
		log(`  ⚠ skipping ${name} — not available on this machine`);
		continue;
	}
	const samples: string[] = [];
	let accepted = 0;
	for (const file of rejected) {
		if (!accepts(impl, file.content)) continue;
		accepted++;
		if (samples.length < SAMPLE_LIMIT) samples.push(file.path);
	}
	results.push({
		name,
		accepted,
		total: rejected.length,
		rate: accepted / rejected.length,
		samples
	});
}

for (const { impl } of rows) impl?.dispose();

// The oracle row must be 0 — see the module doc.
const oracle_row = results.find((r) => r.name === 'svelte/compiler');
const oracle_broken = oracle_row !== undefined && oracle_row.accepted > 0;

if (json_mode) {
	console.log(
		JSON.stringify(
			{
				corpus: 'conformance/css',
				scanned: files.length,
				files: rejected.length,
				oracle_agrees: !oracle_broken,
				rows: results.map(({ name, accepted, total, rate }) => ({ name, accepted, total, rate }))
			},
			null,
			'\t'
		)
	);
} else {
	console.error(
		`\nCSS over-acceptance — ${rejected.length} of ${files.length} files parseCss rejects`
	);
	console.error("(lower is better; parseCss is not a validity oracle, so an accept isn't a bug)\n");
	const width = Math.max(...results.map((r) => r.name.length));
	for (const row of results.sort((a, b) => a.accepted - b.accepted)) {
		const pct = ((row.rate * 100).toFixed(1) + '%').padStart(6);
		console.error(
			`  ${row.name.padEnd(width)}  ${String(row.accepted).padStart(4)}/${row.total}  ${pct}`
		);
		if (verbose && row.samples.length > 0) {
			for (const sample of row.samples) console.error(`      ${sample}`);
		}
	}
	console.error('');
}

if (oracle_broken) {
	console.error(
		`FAIL: the svelte/compiler row accepted ${oracle_row!.accepted} of its own rejects — the ` +
			`oracle disagrees with itself, so these numbers are graded against a moved reference.`
	);
	Deno.exit(1);
}

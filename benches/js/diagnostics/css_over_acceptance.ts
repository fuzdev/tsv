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
 * pins — the ../prettier and ../svelte checkout commits, the wpt-css harvest
 * count, and the svelte oracle version are all pinned elsewhere — so a move means
 * one of them moved, or the oracle's grammar did. Without the pin, a `parseCss`
 * that started rejecting (or accepting) wholesale would silently reshape the
 * published `parse/css` coverage row with nothing to catch it.
 *
 * **Two modes.** The default run is the profile above. `--pin-only` grades the
 * pin and nothing else: it loads only the oracle (no FFI / WASM build, though the
 * CORPUS load is still the whole conformance view — 79.5k files to reach 22.6k CSS,
 * since the loader has no language axis, ~10 s warm) and is
 * FRESHNESS-STAMPED like the suite harvests (`lib/harvest_stamp.ts` — the
 * ../svelte + ../prettier + ../wpt commits, the svelte oracle version, and the two
 * pins that shape the corpus), so a run whose inputs are unchanged skips instantly.
 * That is what lets `bench:pins:suites` carry it as a leg beside the four
 * harvests: the pin is re-derived on the same cadence as its siblings rather than
 * by hand. The list itself is still built live — nothing else consumes it, so a
 * cache would be a second thing to keep fresh; only the stamp is kept.
 * `--if-present` (passed by that task) warn-skips when the CSS corpus is partial
 * (an absent checkout or suite cache, or no sidecar) — the pin is a claim about
 * the FULL corpus, so a smaller one is not a move; a manual run without the flag
 * fails closed, matching the harvests. `--force` re-grades despite a fresh stamp.
 * The profile run writes the same stamp once its own pin check passes.
 *
 * `svelte/compiler` appears as a row and must read 0/N: it BUILT this list, so
 * any other number means the oracle disagrees with itself and the run fails
 * rather than publishing a profile graded against a moved reference.
 *
 * Run (from the repo root):
 *   deno task css:over-acceptance             # the profile (builds FFI + WASM first)
 *   deno task css:over-acceptance --json 2>/dev/null > report.json
 *   deno task css:over-acceptance --verbose   # per-tool sample paths
 *   deno task css:over-acceptance:pin         # the pin alone (a bench:pins:suites leg)
 */

import { CanonicalImplementation } from '../lib/canonical.ts';
import { corpus_missing_entries, DevReposLoader } from '../lib/corpus.ts';
import { CSS_REJECTS_PIN, WPT_CSS_HARVEST_PIN } from '../lib/gate_counts.ts';
import {
	git_head,
	HARVEST_STAMPS,
	harvest_up_to_date,
	short_commit,
	type StampInputs,
	write_stamp
} from '../lib/harvest_stamp.ts';
import type { InitializedImplementations } from '../lib/implementations.ts';
import { CANONICAL_PARSER_ROWS, type TsvImplementation } from '../lib/types.ts';
import { load_all_versions } from '../lib/versions.ts';

/** Sample paths kept per tool for `--verbose` — enough to see the shape, not a dump. */
const SAMPLE_LIMIT = 8;

const json_mode = Deno.args.includes('--json');
const verbose = Deno.args.includes('--verbose') || Deno.args.includes('-v');
const pin_only = Deno.args.includes('--pin-only');
const if_present = Deno.args.includes('--if-present');
const force = Deno.args.includes('--force');
const log = (...args: unknown[]): void => {
	if (!json_mode) console.error(...args);
};

const STAMP_PATH = HARVEST_STAMPS['css-rejects'].path;

// Freshness stamp: the CSS corpus is prettier's suite (../prettier), the svelte
// suite's own `.css` (../svelte) and the wpt-css harvest (../wpt), graded by the
// pinned npm svelte — skip the grade when all of those plus the rejects pin match
// the stamp. Only `--pin-only` skips: the profile is the point of a default run.
//
// Each source checkout is stamped by COMMIT, `../wpt` included. Its harvest count
// pin is stamped too but cannot stand in for the commit: wpt supplies 22310 of the
// 22642 CSS files, and an edit to an existing test moves content without moving the
// count — so a wpt pull would re-run `bench:harvest:wpt`, rewrite the cache, and
// leave this grade stamped fresh over a corpus that changed under it.
const versions = await load_all_versions();
const svelte_commit = git_head('../svelte');
const stamp_inputs: StampInputs = {
	harvest: 'css-rejects',
	svelte_commit,
	prettier_commit: git_head('../prettier'),
	wpt_commit: git_head('../wpt'),
	svelte_oracle: versions.canonical.svelte,
	wpt_pin: WPT_CSS_HARVEST_PIN,
	rejects_pin: CSS_REJECTS_PIN
};
if (
	pin_only &&
	!force &&
	svelte_commit !== null &&
	(await harvest_up_to_date(STAMP_PATH, stamp_inputs, []))
) {
	console.error(
		`css-rejects pin up to date (../svelte at ${short_commit(svelte_commit)}, ` +
			`oracle svelte@${versions.canonical.svelte}, pin ${CSS_REJECTS_PIN}) — skipping; --force to re-grade.`
	);
	Deno.exit(0);
}

/** Warn-and-skip under `--if-present`, fail closed otherwise. */
const skip_or_fail = (msg: string): never => {
	if (if_present) {
		console.error(`  ⚠ ${msg} — skipping (--if-present)`);
		Deno.exit(0);
	}
	console.error(`FAIL: ${msg}`);
	Deno.exit(1);
};

// A pin is a claim about the FULL corpus, so a partial one is asked about before
// anything loads — per language, so an absent test262 cache (JS) is not read as a
// smaller CSS corpus. The profile run leaves this to the loader's own fail-fast.
if (pin_only) {
	const { missing, optional_missing } = await corpus_missing_entries('conformance', 'css');
	const absent = [...missing, ...optional_missing];
	if (absent.length > 0) {
		skip_or_fail(`css-rejects pin: the conformance CSS corpus is partial — ${absent.join(', ')}`);
	}
}

/**
 * The row this comparison's ORACLE reports under. Named once because the run
 * REGISTERS the row below and looks it up again further down — the register /
 * look-up pair is where a respelling goes unnoticed. From the shared constant
 * rather than spelled here, since `parseCss` is the canonical CSS parser the rest
 * of the harness already names that way.
 */
const ORACLE_ROW = CANONICAL_PARSER_ROWS.css;

// The oracle alone for the pin; every impl for the profile. The full set is a
// dynamic import so the pin leg never touches the FFI / WASM loaders it does not
// need (and whose artifacts its task does not build).
let impls: InitializedImplementations | null = null;
let oracle: TsvImplementation;
if (pin_only) {
	const canonical = new CanonicalImplementation(versions.canonical);
	try {
		await canonical.init();
	} catch (e) {
		skip_or_fail(
			`css-rejects pin: could not init svelte/compiler (${e instanceof Error ? e.message : e}) ` +
				'— run `deno task bench:install`'
		);
	}
	oracle = canonical;
} else {
	const { init_implementations } = await import('../lib/implementations.ts');
	impls = await init_implementations({ logger: log });
	oracle = impls.canonical;
}

/** The CSS-parsing rows, in the conformance report's display order. */
const rows: Array<{ name: string; impl: TsvImplementation | undefined }> =
	impls === null
		? [{ name: ORACLE_ROW, impl: oracle }]
		: [
				{ name: ORACLE_ROW, impl: impls.canonical },
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

// Build the reject list with the oracle itself, live (see the module doc on why
// there is no cache).
const rejected = files.filter((f) => !accepts(oracle, f.content));

if (rejected.length !== CSS_REJECTS_PIN) {
	console.error(
		`FAIL: the oracle rejects ${rejected.length} of ${files.length} CSS files ≠ pinned ` +
			`${CSS_REJECTS_PIN}. Either a pinned input moved (../prettier's or ../svelte's checkout, ` +
			`the wpt-css harvest) or svelte's parseCss changed what it accepts — re-pin CSS_REJECTS_PIN ` +
			`in lib/gate_counts.ts deliberately, after checking which.`
	);
	Deno.exit(1);
}
// Stamped only once the pin check passes, so a wrong count never stamps itself fresh.
if (svelte_commit !== null) await write_stamp(STAMP_PATH, stamp_inputs);

if (pin_only) {
	oracle.dispose();
	console.error(
		`css-rejects pin: parseCss rejects ${rejected.length}/${files.length} conformance CSS files ` +
			`— matches CSS_REJECTS_PIN (stamped).`
	);
	Deno.exit(0);
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
const oracle_row = results.find((r) => r.name === ORACLE_ROW);
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

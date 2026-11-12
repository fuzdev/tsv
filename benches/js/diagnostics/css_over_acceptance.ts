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
 * pin and nothing else: it loads only the oracle (no addon / WASM build, though the
 * CORPUS load is still the whole conformance view — 80.4k files to reach 22.4k CSS,
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
 *   npm run css:over-acceptance               # the profile (builds the addon + WASM first)
 *   npm run -s css:over-acceptance -- --json 2>/dev/null > report.json
 *   npm run css:over-acceptance -- --verbose  # per-tool sample paths
 *   npm run css:over-acceptance:pin           # the pin alone (a bench:pins:suites leg)
 *
 * `run_css_over_acceptance` is the library function; its root,
 * `benches/js/roots/css_over_acceptance.ts`, binds its deps (`CssOverAcceptanceDeps`).
 *
 * @module
 */

import { CSS_REJECTS_PIN, WPT_CSS_HARVEST_PIN } from '../lib/gate_counts.ts';
import {
	corpus_filter_fingerprint,
	git_head,
	harvest_up_to_date,
	short_commit,
	type StampGit,
	type StampInputs,
	type StampWrites,
	write_stamp
} from '../lib/harvest_stamp.ts';
import { HARVEST_STAMPS } from '../lib/harvest_stamp_table.ts';
import {
	CANONICAL_PARSER_ROWS,
	type Logger,
	type SourceFile,
	type TsvImplementation
} from '../lib/types.ts';
import { load_all_versions } from '../lib/versions.ts';

/** The implementations a profile run grades, by the slots it reads (`init_implementations`'). */
export interface CssOverAcceptanceImplementations {
	canonical: TsvImplementation;
	native?: TsvImplementation;
	wasm?: TsvImplementation;
	postcss?: TsvImplementation;
}

/** The conformance corpus's CSS, as `lib/corpus.ts` reads it. */
export interface CssCorpus {
	/** The view's absent CSS-bearing entries (`corpus_missing_entries`). */
	missing_entries(
		view: 'conformance',
		language: 'css'
	): Promise<{ missing: string[]; optional_missing: string[] }>;
	/** The view's entry list (`corpus_view_paths`). */
	view_paths(view: 'conformance'): Promise<string[]>;
	/**
	 * The view's whole CSS corpus, refusing a partial one (`load_pinned_language_corpus`);
	 * `null` when `if_present` and an entry is absent.
	 */
	load(
		view: 'conformance',
		language: 'css',
		options: { if_present: boolean; logger: Logger }
	): Promise<SourceFile[] | null>;
}

/** What `run_css_over_acceptance` reaches the world through. */
export interface CssOverAcceptanceDeps {
	/** `git` for the stamp's checkout commits. */
	git: StampGit;
	/** The checkout's files by project-root-relative path; writes only the stamp. */
	files: StampWrites;
	corpus: CssCorpus;
	/** Loads the oracle alone, `svelte/compiler`'s `parseCss`, for the pin leg; throws on failure. */
	init_oracle(): Promise<TsvImplementation>;
	/**
	 * Loads every implementation the profile grades; the pin leg never calls it, so it never
	 * loads an engine it doesn't need.
	 */
	init_implementations(logger: Logger): Promise<CssOverAcceptanceImplementations>;
	/** Where `--json` writes its report. */
	stdout: { write(data: string): Promise<unknown> };
}

/** Sample paths kept per tool for `--verbose` — enough to see the shape, not a dump. */
const SAMPLE_LIMIT = 8;

const STAMP_PATH = HARVEST_STAMPS['css-rejects'].path;

/**
 * The row this comparison's ORACLE reports under. Named once because the run
 * REGISTERS the row below and looks it up again further down — the register /
 * look-up pair is where a respelling goes unnoticed. From the shared constant
 * rather than spelled here, since `parseCss` is the canonical CSS parser the rest
 * of the harness already names that way.
 */
const ORACLE_ROW = CANONICAL_PARSER_ROWS.css;

/** One profile row: how many of the oracle's rejects a tool accepts. */
interface Row {
	name: string;
	accepted: number;
	total: number;
	rate: number;
	samples: string[];
}

const accepts = (impl: TsvImplementation, source: string): boolean => {
	try {
		impl.parse(source, 'css');
		return true;
	} catch {
		return false;
	}
};

/**
 * Grades the CSS reject pin and, unless `--pin-only`, the over-acceptance profile.
 *
 * @param deps - the stamp's git and files, the corpus, the engines, stdout
 * @param argv - the tool's arguments: `--json`, `--verbose`/`-v`, `--pin-only`,
 * `--if-present`, `--force`
 * @returns the exit code: 0 on a pass or a skip, 1 on a moved pin, a partial corpus without
 * `--if-present`, or an oracle row that accepts its own rejects
 */
export async function run_css_over_acceptance(
	deps: CssOverAcceptanceDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	const json_mode = argv.includes('--json');
	const verbose = argv.includes('--verbose') || argv.includes('-v');
	const pin_only = argv.includes('--pin-only');
	const if_present = argv.includes('--if-present');
	const force = argv.includes('--force');
	const log: Logger = (...args) => {
		if (!json_mode) console.error(...args);
	};

	// Freshness stamp: the CSS corpus is prettier's suite (../prettier), the svelte
	// suite's own `.css` (../svelte) and the wpt-css harvest (../wpt), graded by the
	// pinned npm svelte — skip the grade when all of those plus the rejects pin match
	// the stamp. Only `--pin-only` skips: the profile is the point of a default run.
	//
	// Each source checkout is stamped by COMMIT, `../wpt` included. Its harvest count
	// pin is stamped too but cannot stand in for the commit: wpt supplies nearly all of the
	// corpus's CSS files, and an edit to an existing test moves content without moving the
	// count — so a wpt pull would re-run `bench:harvest:wpt`, rewrite the cache, and
	// leave this grade stamped fresh over a corpus that changed under it. The
	// conformance view's ENTRY LIST is stamped too (the whole view's, not only the
	// CSS-bearing entries — one spelling, at the cost of a needless re-grade when a
	// non-CSS suite joins): an entry added to or dropped from the corpus entries changes
	// the graded corpus while every checkout stays put.
	const versions = await load_all_versions();
	const svelte_commit = await git_head(deps.git, '../svelte');
	const stamp_inputs: StampInputs = {
		harvest: 'css-rejects',
		svelte_commit,
		prettier_commit: await git_head(deps.git, '../prettier'),
		wpt_commit: await git_head(deps.git, '../wpt'),
		svelte_oracle: versions.canonical.svelte,
		wpt_pin: WPT_CSS_HARVEST_PIN,
		rejects_pin: CSS_REJECTS_PIN,
		conformance_entries: (await deps.corpus.view_paths('conformance')).join(' '),
		filters: await corpus_filter_fingerprint(deps.files, 'conformance')
	};

	/** Warn-and-skip under `--if-present` (exit 0), fail closed otherwise (exit 1). */
	const skip_or_fail = (msg: string): number => {
		if (if_present) {
			console.error(`  ⚠ ${msg} — skipping (--if-present)`);
			return 0;
		}
		console.error(`FAIL: ${msg}`);
		return 1;
	};

	// Completeness is asked TWICE, and the two askings answer different questions —
	// which is why neither is redundant:
	//
	//  1. HERE, before the stamp: "may I trust the stamp?" The other harvests pass
	//     their own cache to `harvest_up_to_date`, so a wiped cache re-runs them,
	//     while this grade writes no cache to name — a stamp-only test would skip as
	//     "fresh" over a corpus the wpt cache has been deleted out of (`bench:clean`
	//     does exactly that). Only `--pin-only` reaches a stamp, so only it asks here.
	//  2. At LOAD, for both modes: "is what I am about to grade complete?" — the
	//     loader's `{ complete_for: 'css' }` policy below, which refuses an absent CSS
	//     entry whether or not it is `optional`. That one is not skippable and needs
	//     no caller to remember it.
	//
	// Both are per-language, so an absent test262 cache (JS) is never read as a
	// smaller CSS corpus. This one costs a stat per CSS entry.
	if (pin_only) {
		const { missing, optional_missing } = await deps.corpus.missing_entries('conformance', 'css');
		const absent = [...missing, ...optional_missing];
		if (absent.length > 0) {
			return skip_or_fail(
				`css-rejects pin: the conformance CSS corpus is partial — ${absent.join(', ')}`
			);
		}
	}

	if (
		pin_only &&
		!force &&
		svelte_commit !== null &&
		(await harvest_up_to_date(deps.files, STAMP_PATH, stamp_inputs, []))
	) {
		console.error(
			`css-rejects pin up to date (../svelte at ${short_commit(svelte_commit)}, ` +
				`oracle svelte@${versions.canonical.svelte}, pin ${CSS_REJECTS_PIN}) — skipping; --force to re-grade.`
		);
		return 0;
	}

	// The oracle alone for the pin; every impl for the profile, loaded through the dep the
	// pin leg never calls.
	let impls: CssOverAcceptanceImplementations | null = null;
	let oracle: TsvImplementation;
	if (pin_only) {
		try {
			oracle = await deps.init_oracle();
		} catch (e) {
			return skip_or_fail(
				`css-rejects pin: could not init svelte/compiler (${e instanceof Error ? e.message : e}) ` +
					'— run `npm run bench:install`'
			);
		}
	} else {
		impls = await deps.init_implementations(log);
		oracle = impls.canonical;
	}

	// The conformance view is the surface this profile explains — prettier's CSS
	// suite plus the wpt-css harvest, both pinned. The `gates`/`perf` views hold the
	// real-code snapshot, a different question from over-acceptance on suites.
	//
	// The pinned load in BOTH modes, and the profile mode is why it is here rather than
	// folded into the `--pin-only` probe above: the wpt-css cache is an `optional` entry,
	// so an ordinary load warns and carries on, and the pin check below then reports an
	// unharvested cache as `the oracle rejects 22 of 332 … re-pin CSS_REJECTS_PIN`. Every
	// mode of this tool grades that pin, so no mode may load a partial CSS corpus.
	const files = await deps.corpus.load('conformance', 'css', { if_present, logger: log });
	if (files === null) return 0;
	if (files.length === 0) {
		// Unreachable via an absent entry (the loader refuses those above); this catches
		// the other way to get an empty set — every CSS entry present but empty.
		console.error('FAIL: no CSS files in the conformance corpus — every CSS entry is empty.');
		return 1;
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
		return 1;
	}
	// Stamped only once the pin check passes, so a wrong count never stamps itself fresh.
	if (svelte_commit !== null) await write_stamp(deps.files, STAMP_PATH, stamp_inputs);

	if (impls === null) {
		oracle.dispose();
		console.error(
			`css-rejects pin: parseCss rejects ${rejected.length}/${files.length} conformance CSS files ` +
				`— matches CSS_REJECTS_PIN (stamped).`
		);
		return 0;
	}

	/** The CSS-parsing rows, in the conformance report's display order. */
	const rows: Array<{ name: string; impl: TsvImplementation | undefined }> = [
		{ name: ORACLE_ROW, impl: impls.canonical },
		{ name: 'tsv', impl: impls.native },
		{ name: 'tsv-wasm', impl: impls.wasm },
		{ name: 'postcss', impl: impls.postcss }
	];

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

	if (json_mode) {
		await deps.stdout.write(
			JSON.stringify(
				{
					corpus: 'conformance/css',
					scanned: files.length,
					files: rejected.length,
					oracle_agrees: !(oracle_row && oracle_row.accepted > 0),
					rows: results.map(({ name, accepted, total, rate }) => ({ name, accepted, total, rate }))
				},
				null,
				'\t'
			) + '\n'
		);
	} else {
		console.error(
			`\nCSS over-acceptance — ${rejected.length} of ${files.length} files parseCss rejects`
		);
		console.error(
			"(lower is better; parseCss is not a validity oracle, so an accept isn't a bug)\n"
		);
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

	if (oracle_row && oracle_row.accepted > 0) {
		console.error(
			`FAIL: the svelte/compiler row accepted ${oracle_row.accepted} of its own rejects — the ` +
				`oracle disagrees with itself, so these numbers are graded against a moved reference.`
		);
		return 1;
	}
	return 0;
}

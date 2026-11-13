/**
 * Per-tool OVER-ACCEPTANCE over the tsc corpus — the axis a coverage number
 * structurally cannot show.
 *
 * Parse coverage counts accepts, so it can only ever reward permissiveness: a
 * parser that accepted every byte would score 100%. The conformance surface
 * handles that by filtering its corpora to VALID inputs (the Svelte
 * canonical-reject cache, test262's expected-positives, this corpus's tsc-valid
 * list) — which makes the number honest but leaves the opposite question
 * unanswered. This tool asks it: over the files tsc's own PARSER rejects
 * (`.cache/ts_repo_rejects.json`, written by `harvest_ts_repo.ts`), how many does
 * each tool accept?
 *
 * **Read it inverted: lower is better, and 0 is not the target.** An accept here
 * is a tool parsing something the language's own parser refuses — but tsv's
 * deferred-early-error posture is deliberate and documented (`docs/conformance_svelte.md`
 * §TypeScript Corrections, `crates/tsv_ts/CLAUDE.md` §Architecture Position): a
 * parser is allowed to accept a construct and leave the diagnostic to a later
 * layer, which is what tsc's own CHECKER-side grammar checks do. So this is a
 * PROFILE, not a gate — the ranked shape of what each tool defers. The
 * corresponding release gate is `conformance:ts-repo`, which grades tsv alone,
 * per-file, against tsc's baselines with acorn as a sub-label.
 *
 * Which files count as rejects is the harvest's decision and deliberately narrow:
 * tsc's PARSER must reject them. Files tsc parses but whose baselines carry a
 * `TS1xxx` grammar error (~790 of them) are excluded — accepting one is matching
 * tsc's parser, not over-accepting — as are non-UTF-8 files, which are an encoding
 * verdict rather than a grammar one.
 *
 * `tsc` appears as a row and must read 0/N. It is the oracle that BUILT this list,
 * so any other number means the cache and the installed tsc disagree (a stale
 * cache after a tsc bump); the run says so and fails rather than publishing a
 * profile graded against a moved oracle.
 *
 * A crash in either of tsv's rows (`is_tsv_crash`: a native panic or a WASM trap) is neither
 * an accept nor a rejection: the file leaves that row's total, and once the report is out the
 * run lists every crash with its engine and file on stderr, in the comparison tools' panic
 * form (`panic_report_lines`), and fails. The other engines' failures stay rejections: their
 * crashes are no verdict on tsv.
 *
 * A stack-exhaustion `RangeError` is counted as a rejection, not a crash: the same catch takes
 * the oracle's rows and the harness's frames, where it names no tsv bug, and tsv's depth limit
 * (a rejection) keeps its own engines' recursion from it.
 *
 * Run (from the repo root):
 *   npm run ts-repo:over-acceptance
 *   npm run -s ts-repo:over-acceptance -- --json 2>/dev/null > report.json
 *   npm run ts-repo:over-acceptance -- --verbose   # per-tool sample paths
 *
 * `run_ts_repo_over_acceptance` is the library function; its root,
 * `benches/js/roots/ts_repo_over_acceptance.ts`, binds its deps (`TsRepoOverAcceptanceDeps`).
 *
 * @module
 */

import {
	crash_text,
	is_tsv_crash,
	type PanicEntry,
	panic_report_lines
} from '../lib/divergence/panic_errors.ts';
import { TS_REPO_REJECTS_PIN } from '../lib/gate_counts.ts';
import { CANONICAL_PARSER_ROWS, type Logger, type TsvImplementation } from '../lib/types.ts';

/** The cache of tsc's parser rejects, as `harvest_ts_repo.ts` writes it, from the checkout. */
export const TS_REPO_REJECTS_PATH = 'benches/js/.cache/ts_repo_rejects.json';

/** The checkout the cache's paths name, as they spell it. */
export const TS_REPO_CHECKOUT = '../typescript';

/** The implementations the profile grades, by the slots it reads (`init_implementations`'). */
export interface TsRepoOverAcceptanceImplementations {
	canonical: TsvImplementation;
	native?: TsvImplementation;
	wasm?: TsvImplementation;
	tsc?: TsvImplementation;
	oxc?: TsvImplementation;
	oxc_wasm?: TsvImplementation;
	yuku_wasm?: TsvImplementation;
}

/** A directory's files by path beneath it (a gro `FsReadDir` is one). */
export interface TextReader {
	read_text(path: string): Promise<string>;
}

/** What `run_ts_repo_over_acceptance` reaches the world through. */
export interface TsRepoOverAcceptanceDeps {
	/** The checkout, for `TS_REPO_REJECTS_PATH`. */
	checkout: TextReader;
	/** `TS_REPO_CHECKOUT`, for the files the cache lists. */
	typescript: TextReader;
	init_implementations(logger: Logger): Promise<TsRepoOverAcceptanceImplementations>;
	/** Where `--json` writes its report. */
	stdout: { write(data: string): Promise<unknown> };
}

/** Sample paths kept per tool for `--verbose` — enough to see the shape, not a dump. */
const SAMPLE_LIMIT = 8;

/**
 * The row this comparison's ORACLE reports under. Named once because the run
 * REGISTERS the row below and looks it up again further down — the register /
 * look-up pair is where a respelling goes unnoticed. `tsc` rather than the
 * canonical parser here: the corpus is the files tsc's own parser rejects, so tsc
 * is what must score zero (see the module doc).
 */
const ORACLE_ROW = 'tsc';

/** One profile row: how many of tsc's rejects a tool accepts. */
interface Row {
	name: string;
	accepted: number;
	total: number;
	rate: number;
	samples: string[];
}

/**
 * Grades every TS parser over the files tsc's parser rejects.
 *
 * @param deps - the checkout and `../typescript` read-only, the engines, stdout
 * @param argv - the tool's arguments: `--json`, `--verbose`/`-v`
 * @returns the exit code: 0, or 1 on a missing, moved, or unreadable cache, a tsc row that
 * accepts its own rejects, or a crash in a tsv row
 */
export async function run_ts_repo_over_acceptance(
	deps: TsRepoOverAcceptanceDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	const json_mode = argv.includes('--json');
	const verbose = argv.includes('--verbose') || argv.includes('-v');
	const log: Logger = (...args) => {
		if (!json_mode) console.error(...args);
	};

	let paths: string[];
	try {
		paths = JSON.parse(await deps.checkout.read_text(TS_REPO_REJECTS_PATH)) as string[];
	} catch {
		console.error(
			`FAIL: ${TS_REPO_REJECTS_PATH} not found — run \`npm run bench:harvest:ts-repo\` (needs ../typescript).`
		);
		return 1;
	}
	if (paths.length !== TS_REPO_REJECTS_PIN) {
		console.error(
			`FAIL: cache holds ${paths.length} rejects ≠ pinned ${TS_REPO_REJECTS_PIN} — the cache is ` +
				`stale or was written by different grading logic. Re-harvest with ` +
				`\`npm run bench:harvest:ts-repo -- --force\`.`
		);
		return 1;
	}

	const sources = new Map<string, string>();
	const prefix = `${TS_REPO_CHECKOUT}/`;
	for (const path of paths) {
		try {
			if (!path.startsWith(prefix)) throw new Error(`not beneath ${TS_REPO_CHECKOUT}`);
			sources.set(path, await deps.typescript.read_text(path.slice(prefix.length)));
		} catch {
			console.error(
				`FAIL: cannot read ${path} — the ../typescript checkout moved under the cache.`
			);
			return 1;
		}
	}

	const impls = await deps.init_implementations(log);

	/** The TS-parsing rows, in the conformance report's display order; `tsv` marks tsv's engines. */
	const rows: Array<{ name: string; impl: TsvImplementation | undefined; tsv: boolean }> = [
		{ name: ORACLE_ROW, impl: impls.tsc, tsv: false },
		{ name: CANONICAL_PARSER_ROWS.typescript, impl: impls.canonical, tsv: false },
		{ name: 'tsv', impl: impls.native, tsv: true },
		{ name: 'tsv-wasm', impl: impls.wasm, tsv: true },
		{ name: 'oxc-parser', impl: impls.oxc, tsv: false },
		{ name: 'oxc-parser-wasm', impl: impls.oxc_wasm, tsv: false },
		{ name: 'yuku-parser-wasm', impl: impls.yuku_wasm, tsv: false }
	];

	const results: Row[] = [];
	// tsv's crashes, each with its engine and input: graded neither way, and the run fails
	const panics: PanicEntry[] = [];
	for (const { name, impl, tsv } of rows) {
		if (!impl?.supports_parse_language('typescript')) {
			log(`  ⚠ skipping ${name} — not available on this machine`);
			continue;
		}
		const samples: string[] = [];
		let accepted = 0;
		let graded = 0;
		for (const [path, source] of sources) {
			try {
				impl.parse(source, 'typescript');
			} catch (e) {
				if (tsv && is_tsv_crash(e)) {
					panics.push({ label: `${name}: ${path}`, error: crash_text(e) });
				} else {
					// a rejection is the expected outcome here
					graded++;
				}
				continue;
			}
			graded++;
			accepted++;
			if (samples.length < SAMPLE_LIMIT) samples.push(path);
		}
		results.push({
			name,
			accepted,
			total: graded,
			rate: accepted / graded,
			samples
		});
	}

	for (const { impl } of rows) impl?.dispose();

	// The oracle row must be 0 — see the module doc.
	const oracle = results.find((r) => r.name === ORACLE_ROW);

	if (json_mode) {
		await deps.stdout.write(
			JSON.stringify(
				{
					corpus: TS_REPO_REJECTS_PATH,
					files: sources.size,
					oracle_agrees: !(oracle && oracle.accepted > 0),
					rows: results.map(({ name, accepted, total, rate }) => ({ name, accepted, total, rate }))
				},
				null,
				'\t'
			) + '\n'
		);
	} else {
		console.error(`\ntsc-corpus over-acceptance — ${sources.size} files tsc's parser rejects`);
		console.error(
			'(lower is better; an accept is a deferred early error, not necessarily a bug)\n'
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

	// After the report, in its stream's place (stderr; `--json`'s stdout stays one document):
	// a crash is the headline, not a reason to withhold the run's numbers.
	for (const line of panics.length > 0 ? panic_report_lines(panics) : []) console.error(line);

	const oracle_moved = oracle !== undefined && oracle.accepted > 0;
	if (oracle_moved) {
		console.error(
			`FAIL: the tsc row accepted ${oracle.accepted} of its own rejects — the cache and the ` +
				`installed tsc disagree (a tsc bump since the harvest?). Re-harvest with ` +
				`\`npm run bench:harvest:ts-repo -- --force\` before reading these numbers.`
		);
	}
	return panics.length > 0 || oracle_moved ? 1 : 0;
}

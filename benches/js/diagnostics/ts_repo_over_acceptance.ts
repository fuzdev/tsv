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
 * @returns the exit code: 0, or 1 on a missing, moved, or unreadable cache, or a tsc row that
 * accepts its own rejects
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
			`FAIL: ${TS_REPO_REJECTS_PATH} not found — run \`deno task bench:harvest:ts-repo\` (needs ../typescript).`
		);
		return 1;
	}
	if (paths.length !== TS_REPO_REJECTS_PIN) {
		console.error(
			`FAIL: cache holds ${paths.length} rejects ≠ pinned ${TS_REPO_REJECTS_PIN} — the cache is ` +
				`stale or was written by different grading logic. Re-harvest with ` +
				`\`deno task bench:harvest:ts-repo --force\`.`
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

	/** The TS-parsing rows, in the conformance report's display order. */
	const rows: Array<{ name: string; impl: TsvImplementation | undefined }> = [
		{ name: ORACLE_ROW, impl: impls.tsc },
		{ name: CANONICAL_PARSER_ROWS.typescript, impl: impls.canonical },
		{ name: 'tsv', impl: impls.native },
		{ name: 'tsv-wasm', impl: impls.wasm },
		{ name: 'oxc-parser', impl: impls.oxc },
		{ name: 'oxc-parser-wasm', impl: impls.oxc_wasm },
		{ name: 'yuku-parser-wasm', impl: impls.yuku_wasm }
	];

	const results: Row[] = [];
	for (const { name, impl } of rows) {
		if (!impl?.supports_parse_language('typescript')) {
			log(`  ⚠ skipping ${name} — not available on this machine`);
			continue;
		}
		const samples: string[] = [];
		let accepted = 0;
		for (const [path, source] of sources) {
			try {
				impl.parse(source, 'typescript');
				accepted++;
				if (samples.length < SAMPLE_LIMIT) samples.push(path);
			} catch {
				// A rejection is the expected outcome here.
			}
		}
		results.push({
			name,
			accepted,
			total: sources.size,
			rate: accepted / sources.size,
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

	if (oracle && oracle.accepted > 0) {
		console.error(
			`FAIL: the tsc row accepted ${oracle.accepted} of its own rejects — the cache and the ` +
				`installed tsc disagree (a tsc bump since the harvest?). Re-harvest with ` +
				`\`deno task bench:harvest:ts-repo --force\` before reading these numbers.`
		);
		return 1;
	}
	return 0;
}

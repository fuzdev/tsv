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
 *   deno task ts-repo:over-acceptance
 *   deno task ts-repo:over-acceptance --json 2>/dev/null > report.json
 *   deno task ts-repo:over-acceptance --verbose   # per-tool sample paths
 */

import { readFile } from 'node:fs/promises';

import { init_implementations } from '../lib/implementations.ts';
import { TS_REPO_REJECTS_PIN } from '../lib/gate_counts.ts';
import { CANONICAL_PARSER_ROWS, type TsvImplementation } from '../lib/types.ts';

const REJECTS_PATH = 'benches/js/.cache/ts_repo_rejects.json';
/** Sample paths kept per tool for `--verbose` — enough to see the shape, not a dump. */
const SAMPLE_LIMIT = 8;

const json_mode = Deno.args.includes('--json');
const verbose = Deno.args.includes('--verbose') || Deno.args.includes('-v');
const log = (...args: unknown[]): void => {
	if (!json_mode) console.error(...args);
};

let paths: string[];
try {
	paths = JSON.parse(await readFile(REJECTS_PATH, 'utf8')) as string[];
} catch {
	console.error(
		`FAIL: ${REJECTS_PATH} not found — run \`deno task bench:harvest:ts-repo\` (needs ../typescript).`
	);
	Deno.exit(1);
}
if (paths.length !== TS_REPO_REJECTS_PIN) {
	console.error(
		`FAIL: cache holds ${paths.length} rejects ≠ pinned ${TS_REPO_REJECTS_PIN} — the cache is ` +
			`stale or was written by different grading logic. Re-harvest with ` +
			`\`deno task bench:harvest:ts-repo --force\`.`
	);
	Deno.exit(1);
}

const sources = new Map<string, string>();
for (const path of paths) {
	try {
		sources.set(path, await readFile(path, 'utf8'));
	} catch {
		console.error(`FAIL: cannot read ${path} — the ../typescript checkout moved under the cache.`);
		Deno.exit(1);
	}
}

const impls = await init_implementations({ logger: log });

/**
 * The row this comparison's ORACLE reports under. Named once because the run
 * REGISTERS the row below and looks it up again further down — the register /
 * look-up pair is where a respelling goes unnoticed. `tsc` rather than the
 * canonical parser here: the corpus is the files tsc's own parser rejects, so tsc
 * is what must score zero (see the module doc).
 */
const ORACLE_ROW = 'tsc';

/** The TS-parsing rows, in the conformance report's display order. */
const rows: Array<{ name: string; impl: TsvImplementation | undefined }> = [
	{ name: ORACLE_ROW, impl: impls.tsc },
	{ name: CANONICAL_PARSER_ROWS.typescript, impl: impls.canonical },
	{ name: 'tsv', impl: impls.native },
	{ name: 'tsv_wasm', impl: impls.wasm },
	{ name: 'oxc-parser', impl: impls.oxc },
	{ name: 'oxc-parser-wasm', impl: impls.oxc_wasm },
	{ name: 'yuku-parser-wasm', impl: impls.yuku_wasm }
];

interface Row {
	name: string;
	accepted: number;
	total: number;
	rate: number;
	samples: string[];
}

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
const oracle_broken = oracle !== undefined && oracle.accepted > 0;

if (json_mode) {
	console.log(
		JSON.stringify(
			{
				corpus: REJECTS_PATH,
				files: sources.size,
				oracle_agrees: !oracle_broken,
				rows: results.map(({ name, accepted, total, rate }) => ({ name, accepted, total, rate }))
			},
			null,
			'\t'
		)
	);
} else {
	console.error(`\ntsc-corpus over-acceptance — ${sources.size} files tsc's parser rejects`);
	console.error('(lower is better; an accept is a deferred early error, not necessarily a bug)\n');
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
		`FAIL: the tsc row accepted ${oracle!.accepted} of its own rejects — the cache and the ` +
			`installed tsc disagree (a tsc bump since the harvest?). Re-harvest with ` +
			`\`deno task bench:harvest:ts-repo --force\` before reading these numbers.`
	);
	Deno.exit(1);
}

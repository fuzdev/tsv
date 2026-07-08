/**
 * Harvest the canonical-reject cache for the conformance corpus's **Svelte** set:
 * the files `svelte/compiler`'s modern parser rejects. Writes their paths to
 * `benches/js/.cache/svelte_parse_rejects.json`, which `DevReposLoader`
 * (conformance view) then excludes — so the parse-COVERAGE headline measures
 * fidelity on *valid* Svelte, not permissiveness over an adversarial corpus that
 * deliberately bundles error fixtures (svelte's `compiler-errors/`, `loose-*`,
 * preprocess inputs) and non-Svelte HTML (prettier's `tests/format/html`). A file
 * `svelte/compiler` rejects "shouldn't pass" — see internal notes,
 * `TODO_NODE_BENCHMARKS.md` §"Reading the Svelte conformance-coverage number".
 *
 * **Svelte only, by design.** `svelte/compiler` is the one canonical parser tsv is
 * a strict drop-in *for*, so its verdict defines validity. The TS canonical
 * (`acorn-typescript`) is NOT a validity oracle — it *trails* modern TS/JS, so its
 * rejects include valid code tsv (and oxc) correctly parse; excluding those would
 * hide real coverage. CSS's `parseCss` is lenient (over-accepts), likewise not a
 * validity oracle. So neither gets a reject cache; only Svelte does.
 *
 * Machine-local, regenerable (like the wpt/test262 harvest caches): paths are
 * absolute and the cache is gitignored. `--if-present` (passed by the
 * `bench:harvest:svelte-rejects` task) warn-and-skips when `../svelte` or the
 * `node_modules` sidecar is absent, leaving no cache — the loader then fails
 * open to the un-filtered corpus (disclosed in its log). A manual run WITHOUT
 * the flag fails closed instead, matching the wpt/test262 harvests. `--force`
 * re-harvests despite a fresh stamp (default runs skip when the ../svelte +
 * ../prettier commits, the svelte oracle pin, and the rejects pin all match —
 * see `lib/harvest_stamp.ts`). The stamp deliberately ignores the live dev
 * repos in the conformance view: their Svelte is valid by assumption (rejects
 * come from the suite trees), and the exact rejects pin re-validates whenever
 * the harvest does run.
 *
 * Run (from repo root):
 *   deno run --allow-read --allow-write=benches/js/.cache --allow-env --allow-net \
 *     --allow-sys --config benches/js/deno.json \
 *     benches/js/diagnostics/svelte_reject_harvest.ts
 */

import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, relative, resolve } from 'node:path';

import { DevReposLoader } from '../lib/corpus.ts';
import { CanonicalImplementation } from '../lib/canonical.ts';
import { SVELTE_REJECTS_PIN } from '../lib/gate_counts.ts';
import {
	git_head,
	harvest_up_to_date,
	short_commit,
	type StampInputs,
	write_stamp,
} from '../lib/harvest_stamp.ts';
import { load_all_versions } from '../lib/versions.ts';

const CACHE_PATH = 'benches/js/.cache/svelte_parse_rejects.json';
const STAMP_PATH = 'benches/js/.cache/svelte_parse_rejects.stamp.json';
const if_present = Deno.args.includes('--if-present');
const force = Deno.args.includes('--force');

async function main(): Promise<void> {
	const versions = await load_all_versions();

	// Freshness stamp: the graded suite trees come from ../svelte (+ prettier's
	// html suite from ../prettier) and the oracle is the pinned npm svelte —
	// skip the grade when all of those plus the rejects pin match the stamp.
	const svelte_commit = git_head('../svelte');
	const stamp_inputs: StampInputs = {
		harvest: 'svelte-rejects',
		svelte_commit,
		prettier_commit: git_head('../prettier'),
		svelte_oracle: versions.canonical.svelte,
		rejects_pin: SVELTE_REJECTS_PIN,
	};
	if (
		!force && svelte_commit !== null &&
		(await harvest_up_to_date(STAMP_PATH, stamp_inputs, [CACHE_PATH]))
	) {
		console.error(
			`svelte-rejects harvest up to date (../svelte at ${short_commit(svelte_commit)}, ` +
				`oracle svelte@${versions.canonical.svelte}, pin ${SVELTE_REJECTS_PIN}) — skipping; --force to re-harvest.`,
		);
		return;
	}
	const canonical = new CanonicalImplementation(versions.canonical);
	try {
		await canonical.init();
	} catch (e) {
		const msg = `svelte_reject_harvest: could not init svelte/compiler (${e instanceof Error ? e.message : e})`;
		if (if_present) {
			console.error(`  ⚠ ${msg} — skipping (run \`deno task bench:install\`)`);
			return;
		}
		throw new Error(msg);
	}

	// Load the conformance view but only grade Svelte files. `apply_reject_cache:
	// false` is load-bearing — this harvest PRODUCES that cache, so it must see the
	// un-filtered corpus (otherwise it excludes the files it needs to grade and, on
	// a re-run, rewrites the cache empty). `allow_missing` lets a machine without
	// the wpt/test262 suite caches (css/js — no Svelte) still harvest the full
	// Svelte set; a missing ../svelte warn-and-skips under --if-present.
	let files;
	try {
		files = await new DevReposLoader('conformance', {
			allow_missing: true,
			apply_reject_cache: false,
		}).load((m) => console.error(m));
	} catch (e) {
		const msg = `svelte_reject_harvest: could not load conformance corpus (${e instanceof Error ? e.message : e})`;
		if (if_present) {
			console.error(`  ⚠ ${msg} — skipping`);
			return;
		}
		throw new Error(msg);
	}

	const svelte = files.filter((f) => f.language === 'svelte');
	const rejects: string[] = [];
	for (const f of svelte) {
		try {
			canonical.parse(f.content, 'svelte');
		} catch {
			rejects.push(f.path);
		}
	}
	rejects.sort();

	// Pinned count (exact): fewer rejects means the svelte/compiler oracle
	// stopped rejecting (broken import/config); more means it started rejecting
	// wholesale — either way the cache would corrupt the published coverage
	// number. Fail BEFORE writing so a wrong cache never replaces a good one;
	// applies regardless of --if-present (that tolerates a MISSING oracle, not a
	// broken one). See ../lib/gate_counts.ts.
	if (rejects.length !== SVELTE_REJECTS_PIN) {
		console.error(
			`FAIL: pinned count mismatch — ${rejects.length} rejects ≠ pinned ${SVELTE_REJECTS_PIN}; ` +
				`cache not written. If the move is deliberate (suite refresh), re-pin in lib/gate_counts.ts.`,
		);
		Deno.exit(1);
	}

	const out = resolve(CACHE_PATH);
	await mkdir(dirname(out), { recursive: true });
	await writeFile(out, JSON.stringify(rejects, null, '\t') + '\n');
	if (svelte_commit !== null) {
		await write_stamp(STAMP_PATH, stamp_inputs);
	}

	const cwd = resolve('.');
	console.error(
		`svelte_reject_harvest: ${rejects.length}/${svelte.length} Svelte files rejected by ` +
			`svelte/compiler → ${relative(cwd, out)}`,
	);
}

await main();

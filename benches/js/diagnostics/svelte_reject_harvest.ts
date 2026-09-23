/**
 * Harvest the canonical-reject cache for the conformance corpus's **Svelte** set:
 * the files `svelte/compiler`'s modern parser rejects. Writes their paths to
 * `benches/js/.cache/svelte_parse_rejects.json`, which `CorpusLoader`
 * (conformance view) then excludes — so the parse-COVERAGE headline measures
 * fidelity on *valid* Svelte, not permissiveness over an adversarial corpus that
 * deliberately bundles error fixtures (svelte's `compiler-errors/`, `loose-*`,
 * preprocess inputs) and non-Svelte HTML (prettier's `tests/format/html`). A file
 * `svelte/compiler` rejects "shouldn't pass" — how to read the resulting coverage
 * number is `docs/benchmarks.md` §Fairness caveats (Conformance-surface semantics).
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
 * `bench:harvest:svelte-rejects` task) warn-and-skips when the `node_modules`
 * sidecar or ANY Svelte-contributing checkout is absent (`../svelte`,
 * `../prettier`, `../prettier-plugin-svelte` — the refusal comes from the
 * loader's `{ complete_for: 'svelte' }` policy, so the list is derived from
 * the corpus entries rather than restated here), leaving no cache — the loader then
 * fails open to the un-filtered corpus (disclosed in its log), and the coverage run
 * refuses to publish without `BENCH_ALLOW_MISSING=1`. Absence alone
 * (`load_pinned_language_corpus` owns that line): a present input the loader
 * cannot read fails with or without the flag. A manual run
 * WITHOUT the flag fails closed instead, matching the wpt/test262 harvests. `--force`
 * re-harvests despite a fresh stamp (default runs skip when the ../svelte +
 * ../prettier + ../prettier-plugin-svelte commits, the svelte oracle pin, and the
 * rejects pin all match — see `lib/harvest_stamp.ts`; all three checkouts ship
 * Svelte-language corpus, so all three are stamped). The conformance view holds
 * no real code (the snapshot's Svelte is valid by assumption — rejects come from
 * the suite trees), and the exact rejects pin re-validates whenever the harvest
 * does run.
 *
 * Run (from repo root):
 *   deno run --allow-read --allow-write=benches/js/.cache --allow-env --allow-net \
 *     --allow-sys --config benches/js/deno.json \
 *     benches/js/diagnostics/svelte_reject_harvest.ts
 */

import { relative, resolve } from 'node:path';

import { CanonicalImplementation } from '../lib/canonical.ts';
import {
	corpus_view_paths,
	load_pinned_language_corpus,
	SVELTE_REJECT_CACHE
} from '../lib/corpus.ts';
import { SVELTE_REJECTS_PIN } from '../lib/gate_counts.ts';
import {
	corpus_filter_fingerprint,
	git_head,
	HARVEST_STAMPS,
	harvest_up_to_date,
	short_commit,
	type StampInputs,
	write_pinned_path_cache
} from '../lib/harvest_stamp.ts';
import { load_all_versions } from '../lib/versions.ts';

const STAMP_PATH = HARVEST_STAMPS['svelte-rejects'].path;
const if_present = Deno.args.includes('--if-present');
const force = Deno.args.includes('--force');

async function main(): Promise<void> {
	const versions = await load_all_versions();

	// Freshness stamp: the graded Svelte-language trees are ../svelte's tests plus
	// BOTH prettier suites' `.html` (the loader reads that extension as Svelte),
	// and the oracle is the pinned npm svelte — skip the grade when all of those
	// plus the rejects pin match the stamp. All three checkouts are stamped
	// because all three produce rejects (95 / 40 / 7 of the pinned count): a
	// contributor left out is one whose pull leaves this stamp reading fresh over
	// a corpus that moved under it. The conformance view's ENTRY LIST is stamped
	// too: a suite entry added to or dropped from the corpus entries changes what is
	// graded while every checkout stays put, and a stamp keyed on checkouts alone
	// would skip that re-harvest and leave the cache short under a green stamp.
	const svelte_commit = git_head('../svelte');
	const stamp_inputs: StampInputs = {
		harvest: 'svelte-rejects',
		svelte_commit,
		prettier_commit: git_head('../prettier'),
		prettier_plugin_svelte_commit: git_head('../prettier-plugin-svelte'),
		svelte_oracle: versions.canonical.svelte,
		rejects_pin: SVELTE_REJECTS_PIN,
		conformance_entries: (await corpus_view_paths('conformance')).join(' '),
		filters: await corpus_filter_fingerprint('conformance')
	};
	if (
		!force &&
		svelte_commit !== null &&
		(await harvest_up_to_date(STAMP_PATH, stamp_inputs, [SVELTE_REJECT_CACHE]))
	) {
		console.error(
			`svelte-rejects harvest up to date (../svelte at ${short_commit(svelte_commit)}, ` +
				`oracle svelte@${versions.canonical.svelte}, pin ${SVELTE_REJECTS_PIN}) — skipping; --force to re-harvest.`
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

	// Load the conformance view but only grade Svelte files. `apply_exclusion_caches:
	// false` is load-bearing — this harvest PRODUCES one of those caches, so it must
	// see the un-filtered corpus (otherwise it excludes the files it needs to grade
	// and, on a re-run, rewrites the cache empty).
	//
	// `load_pinned_language_corpus` carries the exact tolerance this harvest needs,
	// and saying it any other way has been the bug: a machine without the
	// wpt/test262 suite caches (css/js — no Svelte) still harvests the full Svelte
	// set, while a missing ../svelte / ../prettier / ../prettier-plugin-svelte —
	// each a real contributor to the pinned count (95 / 40 / 7) — refuses HERE,
	// where --if-present can warn-and-skip it. A blanket tolerance instead let those
	// three through to the exact-count check below, which then reported a missing
	// checkout as `pinned count mismatch … re-pin in lib/gate_counts.ts` — the one
	// diagnosis that is never right for an absent input.
	const svelte = await load_pinned_language_corpus('conformance', 'svelte', {
		if_present,
		logger: (m) => console.error(m),
		apply_exclusion_caches: false
	});
	if (svelte === null) return;
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
	// number. See ../lib/gate_counts.ts.
	const out = await write_pinned_path_cache({
		cache: SVELTE_REJECT_CACHE,
		paths: rejects,
		pin: SVELTE_REJECTS_PIN,
		what: 'rejects',
		stamp: svelte_commit === null ? null : { path: STAMP_PATH, inputs: stamp_inputs }
	});

	const cwd = resolve('.');
	console.error(
		`svelte_reject_harvest: ${rejects.length}/${svelte.length} Svelte files rejected by ` +
			`svelte/compiler → ${relative(cwd, out)}`
	);
}

await main();

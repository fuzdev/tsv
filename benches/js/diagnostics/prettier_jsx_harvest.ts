/**
 * Harvest the conformance corpus's **Prettier JSX** exclusion cache: the `.js`
 * fixtures under `../prettier/tests/format/js` that Prettier's own babel parser
 * reads as JSX. Writes their paths to `benches/js/.cache/prettier_jsx_files.json`,
 * which `CorpusLoader` (conformance view) then excludes.
 *
 * Scope, not validity. Prettier's spec verdicts (`lib/prettier_fixtures.ts`)
 * KEEP these files — every parser Prettier verifies a `.js` fixture against takes
 * JSX — but the coverage surface runs every parser in TypeScript mode, where JSX
 * is a syntax error for all of them alike (tsv by design; yuku under `lang: 'ts'`,
 * tsc under `ScriptKind.TS`, acorn-typescript without its JSX plugin). A file
 * every row rejects for the same reason carries no signal and takes the same
 * points off every rate, so it leaves the way the `jsx/` suite and the compiler's
 * `.tsx` cases already have. The oracle is Prettier's babel parser rather than a
 * regex: `<` is also a comparison and a type argument, and only a parse says which.
 *
 * One checkout, one language. Only `../prettier/tests/format/js` holds
 * JS-language fixtures the conformance view keeps (the other suites' `.js` were
 * their `format.test.js` spec files, which the validity filter drops), so the
 * harvest walks that one entry alone (`only`) — not the ~80k files of the whole
 * view, not refusing over a harvest cache its count never reads, and never
 * handing babel a `.ts` fixture it might read as JSX and so drop from the
 * TypeScript suite — and stamps `../prettier` alone.
 *
 * Machine-local, regenerable, `--if-present` / `--force` and the exact-pin
 * posture exactly as `svelte_reject_harvest.ts` — read that header for the
 * tolerance argument (`load_pinned_language_corpus`, `{ complete_for }`).
 *
 * Run (from repo root):
 *   deno run --allow-read --allow-write=benches/js/.cache --allow-env --allow-net \
 *     --allow-sys --config benches/js/deno.json \
 *     benches/js/diagnostics/prettier_jsx_harvest.ts
 */

import { relative, resolve } from 'node:path';

import {
	corpus_view_paths,
	load_pinned_language_corpus,
	PRETTIER_JSX_CACHE
} from '../lib/corpus.ts';
import { PRETTIER_JSX_PIN } from '../lib/gate_counts.ts';
import {
	corpus_filter_fingerprint,
	git_head,
	HARVEST_STAMPS,
	harvest_up_to_date,
	short_commit,
	type StampInputs,
	write_pinned_path_cache
} from '../lib/harvest_stamp.ts';
import { ast_has_jsx } from '../lib/prettier_fixtures.ts';
import { load_all_versions } from '../lib/versions.ts';

const STAMP_PATH = HARVEST_STAMPS['prettier-jsx'].path;
const PRETTIER_JS_SUITE = '../prettier/tests/format/js';
const if_present = Deno.args.includes('--if-present');
const force = Deno.args.includes('--force');

/** The one member of prettier's `__debug` surface this harvest reads. */
interface PrettierDebugParse {
	__debug: {
		parse: (text: string, options: { parser: string }) => Promise<{ ast: unknown }>;
	};
}

async function main(): Promise<void> {
	const versions = await load_all_versions();

	// Freshness stamp: the graded files are one checkout's, the oracle is the pinned
	// npm prettier, and the conformance view's ENTRY LIST is stamped like every
	// other view-loading grade (see `svelte_reject_harvest.ts`).
	const prettier_commit = git_head('../prettier');
	const stamp_inputs: StampInputs = {
		harvest: 'prettier-jsx',
		prettier_commit,
		prettier_oracle: versions.canonical.prettier,
		jsx_pin: PRETTIER_JSX_PIN,
		conformance_entries: (await corpus_view_paths('conformance')).join(' '),
		filters: await corpus_filter_fingerprint('conformance')
	};
	if (
		!force &&
		prettier_commit !== null &&
		(await harvest_up_to_date(STAMP_PATH, stamp_inputs, [PRETTIER_JSX_CACHE]))
	) {
		console.error(
			`prettier-jsx harvest up to date (../prettier at ${short_commit(prettier_commit)}, ` +
				`oracle prettier@${versions.canonical.prettier}, pin ${PRETTIER_JSX_PIN}) — skipping; --force to re-harvest.`
		);
		return;
	}

	let prettier: PrettierDebugParse;
	try {
		prettier = (await import('prettier')) as unknown as PrettierDebugParse;
	} catch (e) {
		const msg = `prettier_jsx_harvest: could not import prettier (${e instanceof Error ? e.message : e})`;
		if (if_present) {
			console.error(`  ⚠ ${msg} — skipping (run \`deno task bench:install\`)`);
			return;
		}
		throw new Error(msg);
	}

	// The un-filtered view (`apply_exclusion_caches: false` — this harvest PRODUCES
	// one of those caches), one language, the Prettier JS suite alone, under the pin
	// posture over that entry.
	const files = await load_pinned_language_corpus('conformance', 'typescript', {
		if_present,
		logger: (m) => console.error(m),
		apply_exclusion_caches: false,
		only: (entry_path) => entry_path === PRETTIER_JS_SUITE
	});
	if (files === null) return;

	const jsx: string[] = [];
	for (const f of files) {
		let ast: unknown;
		try {
			({ ast } = await prettier.__debug.parse(f.content, { parser: 'babel' }));
		} catch {
			// babel rejects it outright (a legacy `assert {}`, an HTML-like comment):
			// not JSX, and the spec verdict has already decided whether it stays.
			continue;
		}
		if (ast_has_jsx(ast)) jsx.push(f.path);
	}
	jsx.sort();

	// Pinned count (exact): see ../lib/gate_counts.ts.
	const out = await write_pinned_path_cache({
		cache: PRETTIER_JSX_CACHE,
		paths: jsx,
		pin: PRETTIER_JSX_PIN,
		what: 'JSX files',
		stamp: prettier_commit === null ? null : { path: STAMP_PATH, inputs: stamp_inputs }
	});

	const cwd = resolve('.');
	console.error(
		`prettier_jsx_harvest: ${jsx.length}/${files.length} Prettier JS fixtures read as JSX by ` +
			`prettier's babel parser → ${relative(cwd, out)}`
	);
}

await main();

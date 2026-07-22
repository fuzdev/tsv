/**
 * Behavioral fixture-coverage audit for divergence patterns.
 *
 * `validation.ts`'s bookkeeping half cross-references each pattern's
 * hand-maintained `fixtures: []` array against `conformance_prettier.md` — it
 * never runs `detect()` for THAT question. A pattern that silently stopped
 * matching its claimed fixture keeps that cross-reference green. This test
 * closes the drift gap by exercising every detector against its own committed
 * fixtures.
 *
 * The machinery — how a fixture's committed files pin a (ours, prettier) pair,
 * and the precedence chain among them — lives in `fixture_cases.ts`, shared
 * with the audit's empirical coverage pass. This file is the per-pattern
 * assertion built on it, plus the unit tests pinning that machinery.
 *
 * The assertion is COVERAGE, not per-witness exhaustiveness: a fixture passes
 * when at least one of its cases is claimed. Requiring every case would assert
 * something the fixtures do not mean — sibling variants deliberately exercise
 * different authorings (`scope_complex` pins three), and a pattern explaining
 * the canonical one is exactly the traceability this audit is checking.
 *
 * A pattern whose every listed fixture yields zero cases would make its test
 * vacuous — passing with no assertion executed — so that is itself a failure
 * (see the coverage assertion at the end of each test).
 *
 * Runs read-only (`--allow-read`).
 */

import { ok as assert } from 'node:assert';
import { detect_divergences, PATTERNS } from './patterns.ts';
import {
	build_cases,
	build_context,
	fixture_dir_exists,
	type FixtureIo,
	select_witnesses,
} from './fixture_cases.ts';

/**
 * Acknowledged ratchet exceptions: (pattern, fixture) pairs where the detector
 * does NOT claim a hunk in a fixture it lists. EMPTY — every pattern now detects
 * every fixture it claims, so any drift hard-fails below.
 *
 * Do NOT add a pair here to silence a regression from tightening a guard — fix
 * the guard. And do not list a fixture under a pattern that cannot detect it:
 * the "Prettier drops the comment" preservation divergences (Svelte
 * `expr_trailing` / `debug_comment`) are intentionally NOT claimed by
 * `comment_position` — its content guard requires the comment in both outputs,
 * so a dropped comment can't (and mustn't) match. Such fixtures stay unclaimed
 * and surface in `divergence:audit` as honestly uncovered rather than being
 * forced into this allowlist.
 *
 * Each entry is `${pattern_id}\t${fixture_path}`.
 */
const PRE_EXISTING_DRIFT = new Set<string>([]);

/**
 * Whether the runner granted `--allow-read`. The wired `test:deno` task grants it,
 * so the audit runs. If you invoke `deno test` manually without `--allow-read`,
 * every test announces (loudly) that it was skipped for lack of read access rather
 * than silently passing with zero detectors exercised — add `--allow-read` to
 * actually exercise the audit.
 */
async function has_read_access(): Promise<boolean> {
	const status = await Deno.permissions.query({ name: 'read' });
	return status.state === 'granted';
}

// ── Witness selection (pure; no fixtures on disk involved) ──────────────────
//
// The precedence chain is the part of this audit most able to be quietly wrong:
// pick the wrong rung and the audit still passes, just asserting something the
// fixture never meant. These pin each rung, the precedence between them, and the
// two refusals.

const svelte_ext = '.svelte';

Deno.test('witnesses: output_prettier outranks every other form', () => {
	const { rung, witnesses } = select_witnesses(
		[
			'input.svelte',
			'output_prettier.svelte',
			'prettier_variant_a.svelte',
			'prettier_intermediate_a.svelte',
			'divergent_variant_a.svelte',
			'variant_a.svelte',
			'unformatted_ours_a.svelte',
		],
		svelte_ext,
	);
	assert(rung === 'output_prettier', `expected output_prettier rung, got ${rung}`);
	assert(witnesses.length === 1 && witnesses[0].prettier_file === 'output_prettier.svelte');
	// prettier's output on `input`, so `input` is also the source
	assert(witnesses[0].source_file === null);
});

Deno.test('witnesses: every prettier_variant is a witness, and is its own source', () => {
	const { rung, witnesses } = select_witnesses(
		['input.svelte', 'prettier_variant_b.svelte', 'prettier_variant_a.svelte'],
		svelte_ext,
	);
	assert(rung === 'prettier_variant', `expected prettier_variant rung, got ${rung}`);
	assert(witnesses.length === 2, `expected both variants, got ${witnesses.length}`);
	// prettier KEEPS a variant stable (N1), so it is both the source and the output
	for (const w of witnesses) assert(w.source_file === w.prettier_file);
});

Deno.test('witnesses: prettier_intermediate stands in when no stable form exists', () => {
	const { rung, witnesses } = select_witnesses(
		['input.svelte', 'prettier_intermediate_parens.svelte', 'unformatted_ours_parens.svelte'],
		svelte_ext,
	);
	assert(rung === 'prettier_intermediate', `expected prettier_intermediate rung, got ${rung}`);
	assert(witnesses.length === 1);
});

Deno.test('witnesses: divergent_variant outranks the N10 inference', () => {
	// The `last_block` shape: prettier's output is stated by divergent_variant,
	// so inferring a different one from the lone `variant_*` would be wrong.
	const { rung, witnesses } = select_witnesses(
		[
			'input.svelte',
			'divergent_variant_last_inline.svelte',
			'variant_expanded_last_glued.svelte',
			'unformatted_ours_spaces.svelte',
		],
		svelte_ext,
	);
	assert(rung === 'divergent_variant', `expected divergent_variant rung, got ${rung}`);
	assert(witnesses[0].prettier_file === 'divergent_variant_last_inline.svelte');
});

Deno.test('witnesses: N10 fires on exactly one stable form', () => {
	const { rung, witnesses } = select_witnesses(
		['input.svelte', 'variant_standalone.svelte', 'unformatted_ours_standalone.svelte'],
		svelte_ext,
	);
	assert(rung === 'n10', `expected n10 rung, got ${rung}`);
	assert(witnesses[0].prettier_file === 'variant_standalone.svelte');
	assert(witnesses[0].source_file === 'unformatted_ours_standalone.svelte');
});

Deno.test('witnesses: N10 refuses two stable forms rather than guessing', () => {
	const { rung, witnesses } = select_witnesses(
		[
			'input.svelte',
			'variant_a.svelte',
			'variant_b.svelte',
			'unformatted_ours_spaces.svelte',
		],
		svelte_ext,
	);
	assert(rung === 'none', `ambiguous stable set must yield no witness, got ${rung}`);
	assert(witnesses.length === 0);
});

Deno.test('witnesses: N10 needs an unformatted_ours authoring to pair against', () => {
	const { rung } = select_witnesses(['input.svelte', 'variant_a.svelte'], svelte_ext);
	assert(rung === 'none', `a lone variant pins no prettier output, got ${rung}`);
});

Deno.test('witnesses: a bare divergence fixture yields nothing', () => {
	const { rung, witnesses } = select_witnesses(['input.svelte', 'README.md'], svelte_ext);
	assert(rung === 'none');
	assert(witnesses.length === 0);
});

Deno.test('witnesses: extension match is exact, so .svelte ignores .svelte.ts siblings', () => {
	// A `.svelte` fixture must not claim a `.svelte.ts` variant, nor vice versa —
	// `endsWith` on the full suffix is what keeps the two apart.
	const svelte = select_witnesses(
		['input.svelte', 'prettier_variant_a.svelte.ts'],
		'.svelte',
	);
	assert(svelte.rung === 'none', `.svelte must not claim a .svelte.ts variant`);

	const svelte_ts = select_witnesses(
		['input.svelte.ts', 'prettier_variant_a.svelte', 'prettier_variant_b.svelte.ts'],
		'.svelte.ts',
	);
	assert(svelte_ts.rung === 'prettier_variant');
	assert(svelte_ts.witnesses.length === 1, 'only the .svelte.ts variant counts');
	assert(svelte_ts.witnesses[0].prettier_file === 'prettier_variant_b.svelte.ts');
});

// ── Case assembly (in-memory fixture; no disk involved) ─────────────────────
//
// `select_witnesses` decides WHICH files speak; this decides which content lands
// in `source` / `ours` / `prettier`. Getting that wiring wrong is silent — the
// audit would still run, just comparing the wrong pair — so it is pinned here.

/** An in-memory `FixtureIo` over `{filename: content}`. */
const memory_io = (files: Record<string, string>): FixtureIo => ({
	list: () => Promise.resolve(Object.keys(files).sort()),
	read: (_path, name) => Promise.resolve(files[name] ?? null),
});

Deno.test('assembly: ours is always input, prettier is the witness content', async () => {
	const cases = await build_cases(
		'x',
		memory_io({ 'input.svelte': 'OURS', 'output_prettier.svelte': 'PRETTIER' }),
	);
	assert(cases !== 'no_input' && cases.length === 1);
	assert(cases[0].ours === 'OURS', `ours must be input, got ${cases[0].ours}`);
	assert(cases[0].prettier === 'PRETTIER', `prettier must be the witness`);
	// output_prettier is prettier's output ON input, so input is also the source
	assert(cases[0].source === 'OURS');
});

Deno.test('assembly: a prettier_variant is its own source, not input', async () => {
	const cases = await build_cases(
		'x',
		memory_io({ 'input.svelte': 'OURS', 'prettier_variant_a.svelte': 'VARIANT' }),
	);
	assert(cases !== 'no_input' && cases.length === 1);
	assert(cases[0].source === 'VARIANT', `source must be the variant, got ${cases[0].source}`);
	assert(cases[0].ours === 'OURS' && cases[0].prettier === 'VARIANT');
});

Deno.test('assembly: the N10 case reads source and prettier from DIFFERENT files', async () => {
	// The wiring most able to be silently transposed: prettier's output is the
	// stable form, while the source is the authoring being normalized.
	const cases = await build_cases(
		'x',
		memory_io({
			'input.svelte': 'OURS',
			'variant_standalone.svelte': 'STABLE',
			'unformatted_ours_standalone.svelte': 'AUTHORING',
		}),
	);
	assert(cases !== 'no_input' && cases.length === 1);
	assert(cases[0].source === 'AUTHORING', `source must be the authoring, got ${cases[0].source}`);
	assert(cases[0].prettier === 'STABLE', `prettier must be the stable form`);
	assert(cases[0].ours === 'OURS');
});

Deno.test('assembly: no input.* yields no_input regardless of other files', async () => {
	const cases = await build_cases(
		'x',
		memory_io({ 'output_prettier.svelte': 'P', 'README.md': 'r' }),
	);
	assert(cases === 'no_input', `expected no_input, got ${JSON.stringify(cases)}`);
});

Deno.test('assembly: language follows the input extension', async () => {
	const svelte = await build_cases(
		'x',
		memory_io({ 'input.svelte': 'a', 'output_prettier.svelte': 'b' }),
	);
	const ts = await build_cases('x', memory_io({ 'input.ts': 'a', 'output_prettier.ts': 'b' }));
	const css = await build_cases('x', memory_io({ 'input.css': 'a', 'output_prettier.css': 'b' }));
	assert(svelte !== 'no_input' && svelte[0].language === 'svelte');
	assert(ts !== 'no_input' && ts[0].language === 'typescript');
	assert(css !== 'no_input' && css[0].language === 'css');
});

Deno.test('assembly: input resolution order prefers .svelte over .svelte.ts', async () => {
	// INPUT_NAMES order decides the extension, which decides which variants match.
	const cases = await build_cases(
		'x',
		memory_io({
			'input.svelte': 'SVELTE',
			'input.svelte.ts': 'SVELTE_TS',
			'output_prettier.svelte': 'P',
		}),
	);
	assert(cases !== 'no_input' && cases[0].ours === 'SVELTE');
});

/**
 * Listed fixtures whose hunks the pattern set explains only PARTLY, with the
 * hunk count left over and what the shortfall is.
 *
 * A ratchet, in this repo's usual shape: every entry is a known gap, the file
 * shrinking is the goal, and it mirrors the live set exactly — a listed fixture
 * that goes partial without an entry FAILS, and an entry that no longer fires
 * FAILS too (delete it; the detector was widened).
 *
 * These are not mystery divergences. In every case a pattern DOES explain the
 * fixture's divergence and leaves an adjacent hunk unclaimed — the diff splits
 * one logical change across hunk boundaries (a dangling `) {` line), or the
 * detector claims some instances of a repeated divergence but not all (the CSS
 * `||` fixture pins four identical combinators; the pattern claims three).
 * They matter because the corpus classifies by the same rule: such a file lands
 * in the pinned `partial` bucket rather than `known`, so closing one of these
 * is a real corpus-triage win. See `docs/divergence_detector.md` §Pending work.
 */
const KNOWN_PARTIAL: Record<string, string> = {
	// detector claims 3 of 4 identical `||` combinator hunks; misses the compound-selector one
	'css/selectors/combinators/column_prettier_divergence': '1 hunk',
	'css/at_rules/container_spacing_prettier_divergence': '2 hunks',
	// the `{#…}` head reflow splits across hunks; short_expr_100/fill_101_boundary claim the heads
	'svelte/blocks/await/long_prettier_divergence': '2 hunks',
	'svelte/blocks/each/long_prettier_divergence': '2 hunks',
	'svelte/blocks/if/long_prettier_divergence': '3 hunks',
	'svelte/blocks/key/long_prettier_divergence': '2 hunks',
	'svelte/syntax/comments/expr_trailing_line_prettier_divergence': '1 hunk',
	// comment_position claims the comment hunk, not the reflow tail it sits in
	'typescript/expressions/calls/chained/trailing_member_comment_prettier_divergence': '2 hunks',
	'typescript/statements/switch/case_block_comment_prettier_divergence': '1 hunk',
	'typescript/statements/switch/discriminant_trailing_comment_prettier_divergence': '1 hunk',
};

/**
 * Every listed fixture must be FULLY explained by the pattern set.
 *
 * Separate from the per-pattern tests above, and necessarily so: `all_explained`
 * is a set-cover across ALL patterns (two patterns may jointly explain one
 * fixture), so it cannot be asserted per pattern without distorting the
 * per-pattern question, which is only "does this detector still claim its own
 * fixture?".
 *
 * Why this is a stronger bar than that per-pattern one: "claims a hunk" cannot
 * see a hunk left over. Fourteen listed fixtures were partial when this gate was
 * added, sitting inside the curated assertions undetected — the same masking the
 * hunk-aware classifier exists to prevent (a file with one explained and one
 * unexplained hunk is `partial`, not `known`), one level up.
 */
Deno.test('fixture coverage: every listed fixture is FULLY explained', async () => {
	if (!(await has_read_access())) {
		console.warn('[fixture coverage] SKIPPED full-coverage check: no --allow-read');
		return;
	}

	const listed = new Set<string>();
	for (const pattern of PATTERNS) for (const f of pattern.fixtures ?? []) listed.add(f);

	const newly_partial: string[] = [];
	const still_partial = new Set<string>();
	for (const fixture_path of [...listed].sort()) {
		const cases = await build_cases(fixture_path);
		// A path with no directory / no input, or one pinning no prettier form, is
		// the other tests' business — silence here rather than double-reporting.
		if (cases === 'no_input' || cases.length === 0) continue;

		// Coverage across witnesses: the best classification wins, since sibling
		// variants exercise different authorings and a variant's own quirks can add
		// hunks the fixture's divergence never meant to pin.
		let best = 'none_explained';
		let unexplained = 0;
		for (const detection_case of cases) {
			const result = detect_divergences(build_context(detection_case));
			if (result.classification === 'all_explained') {
				best = 'all_explained';
				break;
			}
			if (result.classification === 'partial' && best === 'none_explained') {
				best = 'partial';
				unexplained = result.unexplained_hunks.length;
			}
		}
		if (best === 'all_explained') continue;

		if (fixture_path in KNOWN_PARTIAL) still_partial.add(fixture_path);
		else if (best === 'partial') {
			newly_partial.push(`${fixture_path}  (${unexplained} hunk(s) unexplained)`);
		}
		// `none_explained` is the per-pattern tests' failure to report, not this one's.
	}

	assert(
		newly_partial.length === 0,
		`${newly_partial.length} listed fixture(s) went PARTIAL — a pattern claims some hunks ` +
			`and leaves others unexplained, which the per-pattern "claims a hunk" bar cannot see. ` +
			`Widen the detector, or add a KNOWN_PARTIAL entry with the reason:\n  ` +
			newly_partial.join('\n  '),
	);

	const stale = Object.keys(KNOWN_PARTIAL).filter((f) => !still_partial.has(f));
	assert(
		stale.length === 0,
		`${stale.length} KNOWN_PARTIAL entr(ies) no longer fire — the fixture is now fully ` +
			`explained (or no longer listed). Delete them; the ratchet must mirror the live set:\n  ` +
			stale.join('\n  '),
	);
});

/**
 * Every listed fixture path must name a real directory.
 *
 * Its own test, ahead of the per-pattern ones, for three reasons: it covers
 * EVERY pattern (the per-pattern tests skip empty `fixtures` arrays), it reports
 * every broken reference at once instead of aborting on the first, and it is the
 * gate for a class `divergence:audit` reports but cannot enforce — that audit is
 * not part of `deno task check`, and until it grew `missing_pattern_fixtures` it
 * folded broken references in among the claimed-but-undocumented orphans, where
 * eight of them sat unnoticed.
 */
Deno.test('fixture coverage: every listed fixture path resolves on disk', async () => {
	if (!(await has_read_access())) {
		console.warn('[fixture coverage] SKIPPED path check: no --allow-read');
		return;
	}
	const broken: string[] = [];
	for (const pattern of PATTERNS) {
		for (const fixture_path of pattern.fixtures ?? []) {
			if (!fixture_dir_exists(fixture_path)) broken.push(`${pattern.id} → ${fixture_path}`);
		}
	}
	assert(
		broken.length === 0,
		`${broken.length} pattern fixture listing(s) point at no directory — fix the path ` +
			`or unlist it (a renamed fixture, or one that lost its _prettier_divergence ` +
			`suffix when its divergence was resolved):\n  ${broken.join('\n  ')}`,
	);
});

// One test per pattern (with a non-empty fixtures array). The test name carries
// the pattern id so a failure pinpoints the (pattern, fixture) pair.
for (const pattern of PATTERNS) {
	if (!pattern.fixtures || pattern.fixtures.length === 0) continue;

	Deno.test(`fixture coverage: ${pattern.id} detects its claimed fixtures`, async () => {
		if (!(await has_read_access())) {
			console.warn(
				`[fixture coverage] SKIPPED ${pattern.id}: no --allow-read. The wired ` +
					`test:deno task grants it; you're running deno test without it, so this ` +
					`audit does nothing here. Add \`--allow-read\` (or run \`deno task test:deno\`) ` +
					`to actually exercise the detectors against their fixtures.`,
			);
			return;
		}
		let cases_run = 0;
		for (const fixture_path of pattern.fixtures) {
			const cases = await build_cases(fixture_path);

			if (cases === 'no_input') {
				// A directory that exists but holds no input.*. A path with no
				// DIRECTORY is caught by the broken-reference test below, which
				// reports every such listing at once rather than aborting here on
				// the first one.
				assert(
					false,
					`${pattern.id} lists ${fixture_path}, whose directory holds no input.* file`,
				);
			}
			if (cases.length === 0) {
				console.warn(
					`[fixture coverage] ${pattern.id}: ${fixture_path} pins no prettier form ` +
						`(no output_prettier.*, no prettier_variant_*, and not an unambiguous ` +
						`N10 case) — skipping`,
				);
				continue;
			}

			// Coverage, not exhaustiveness — one claimed witness covers the fixture.
			const claimed: string[] = [];
			for (const detection_case of cases) {
				cases_run++;
				const match = pattern.detect(build_context(detection_case));
				if (match !== null && match.hunk_indices.length > 0) claimed.push(detection_case.label);
			}

			const key = `${pattern.id}\t${fixture_path}`;
			if (PRE_EXISTING_DRIFT.has(key)) {
				// TODO: pre-existing drift — pattern does not detect claimed fixture.
				// Soft warning keeps the suite green while the drift stays visible.
				if (claimed.length === 0) {
					console.warn(
						`[fixture coverage] PRE-EXISTING DRIFT: ${pattern.id} does not detect ` +
							`its claimed fixture ${fixture_path}`,
					);
				} else {
					// The drift was fixed elsewhere — flag so the entry can be removed.
					console.warn(
						`[fixture coverage] ${pattern.id} now DETECTS ${fixture_path} ` +
							`(via ${claimed.join(', ')}); remove it from PRE_EXISTING_DRIFT`,
					);
				}
				continue;
			}

			assert(
				claimed.length > 0,
				`${pattern.id} claims no hunk in any prettier form its own fixture ` +
					`${fixture_path} pins (tried: ${cases.map((c) => c.label).join(', ')})`,
			);
		}

		// A pattern every one of whose fixtures skipped would pass having asserted
		// nothing — the vacuous pass this audit exists to prevent.
		assert(
			cases_run > 0,
			`${pattern.id} ran zero detection cases — every fixture it lists was skipped, ` +
				`so this test asserts nothing. List a fixture that pins a prettier form, or ` +
				`drop the fixtures array.`,
		);
	});
}

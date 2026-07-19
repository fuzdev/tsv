/**
 * Behavioral fixture-coverage audit for divergence patterns.
 *
 * `validation.ts` only cross-references each pattern's hand-maintained
 * `fixtures: []` array against `conformance_prettier.md` — it never runs
 * `detect()` against the real fixtures. A pattern that silently stopped
 * matching its claimed fixture keeps that static audit green. This test closes
 * that drift gap by exercising every detector against its own committed
 * fixtures.
 *
 * Key insight: for a `_prettier_divergence` fixture the committed `input.*`
 * file IS our formatter's output (our formatter is idempotent — input formats
 * to itself). So whenever a committed file pins what *prettier* produces, the
 * pair (ours = input, prettier = that file) is a real divergence the detector
 * must claim — and we can drive it from the committed files alone, with no
 * build and no sidecar.
 *
 * Three committed files can pin a prettier form. They are a PRECEDENCE chain,
 * not a union (see `build_cases`) — each rung is used only when the rung above
 * it is absent:
 *
 * 1. `output_prettier.*` — prettier's output on `input` directly. The canonical
 *    witness, and the only one whose diff against `input` isolates the
 *    fixture's own divergence.
 * 2. every `prettier_variant_*` — a form prettier keeps stable (validator rule
 *    N1) and ours maps to `input` (N2). Prettier's output for THAT authoring,
 *    so the pair is real — but the authoring carries its own quirks (an
 *    intentionally space-mangled variant diffs against `input` on whitespace as
 *    well as on the divergence), which is why these stand in only when there is
 *    no `output_prettier.*` to ask instead.
 * 3. every `prettier_intermediate_*` — prettier's FIRST-pass output on an
 *    `unformatted_ours_*` authoring. It is unstable (a second prettier pass
 *    moves it), but the corpus comparison runs prettier exactly once, so this
 *    is precisely the prettier side a corpus divergence is computed from.
 * 4. every `divergent_variant_*` — a form prettier keeps stable that ours
 *    rewrites to a distinct third form. Prettier's output on the fixture's
 *    `unformatted_ours_*` authoring, stated outright rather than inferred.
 * 5. the N10 fallback, when none of the above exists: a fixture with
 *    `unformatted_ours_*` files and exactly ONE documented stable form
 *    (`variant_*`). N6 pins `prettier(unformatted_ours_X) != input` and N10
 *    pins it to one of the fixture's documented stable forms — with exactly
 *    one such form, that IS prettier's output, so (ours = input,
 *    prettier = the variant) is pinned. Ambiguous with 2+ stable forms, so
 *    those fixtures yield no case rather than a guessed one.
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
import { diff_lines, extract_hunks } from '../diff.ts';
import { type DetectionContext, enrich_detection_context, PATTERNS } from './patterns.ts';
import { fixture_dir_exists } from './validation.ts';
import type { Language } from '../types.ts';

const FIXTURES_ROOT = new URL('../../../../tests/fixtures/', import.meta.url);

/** Candidate input filenames, in resolution order. */
const INPUT_NAMES = ['input.svelte', 'input.svelte.ts', 'input.ts', 'input.css'];

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
 * Read a file, returning null only when it genuinely does not exist. Any other
 * error (notably a permission error) re-throws — a denied read must never be
 * mistaken for a missing fixture file, which would silently hollow out the
 * audit. The suite gates on read permission up front (see `has_read_access`).
 */
async function read_if_exists(url: URL): Promise<string | null> {
	try {
		return await Deno.readTextFile(url);
	} catch (err) {
		if (err instanceof Deno.errors.NotFound) return null;
		throw err;
	}
}

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

function language_of(input_name: string): Language {
	if (input_name.endsWith('.css')) return 'css';
	if (input_name.endsWith('.svelte.ts')) return 'typescript';
	if (input_name.endsWith('.ts')) return 'typescript';
	return 'svelte';
}

/**
 * The extension an input filename carries, as the variant files in the same
 * fixture spell it. Matched with a full-suffix `endsWith`, so `.svelte` and
 * `.svelte.ts` fixtures never claim each other's variants.
 */
function extension_of(input_name: string): string {
	return input_name.slice('input'.length);
}

/** One (ours, prettier) pairing a fixture pins, and the file that pins it. */
interface DetectionCase {
	/** The committed file this pairing came from — named in failure messages. */
	label: string;
	source: string;
	ours: string;
	prettier: string;
	language: Language;
}

/**
 * The filesystem surface case assembly needs.
 *
 * Injected rather than called directly so the assembly — which witness content
 * lands in `source` vs `ours` vs `prettier` — is testable from an in-memory
 * fixture. The suite runs `--allow-read` only (deliberately: it must never be
 * able to mutate the tree it audits), so a temp-directory fixture is not an
 * option, and coupling the test to real fixture directories would make it
 * brittle against exactly the renames this audit exists to catch.
 */
interface FixtureIo {
	/** Filenames directly inside the fixture directory; empty when it is absent. */
	list: (fixture_path: string) => Promise<string[]>;
	/** File content, or null when the file does not exist. */
	read: (fixture_path: string, name: string) => Promise<string | null>;
}

/** The real filesystem, rooted at `tests/fixtures/`. */
const disk_io: FixtureIo = {
	list: async (fixture_path) => {
		const names: string[] = [];
		try {
			for await (const entry of Deno.readDir(new URL(fixture_path + '/', FIXTURES_ROOT))) {
				if (entry.isFile) names.push(entry.name);
			}
		} catch (err) {
			if (err instanceof Deno.errors.NotFound) return names;
			throw err;
		}
		return names.sort();
	},
	read: (fixture_path, name) => read_if_exists(new URL(name, new URL(fixture_path + '/', FIXTURES_ROOT))),
};

/** Which rung of the precedence chain supplied a fixture's witnesses. */
type WitnessRung =
	| 'output_prettier'
	| 'prettier_variant'
	| 'prettier_intermediate'
	| 'divergent_variant'
	| 'n10'
	| 'none';

/** One witness: the file holding prettier's output, and the authoring it came from. */
interface Witness {
	/** File whose CONTENT is prettier's output. */
	prettier_file: string;
	/**
	 * File whose content is the `source` of the pairing. `null` means `input`
	 * itself; `prettier_variant` uses the variant (prettier's own fixed point),
	 * and the N10 rung uses the `unformatted_ours_*` authoring being normalized.
	 */
	source_file: string | null;
	/** Label for failure messages. */
	label: string;
}

/**
 * Pick a fixture's prettier witnesses from its directory listing — the whole
 * precedence chain, as a pure function of the filenames.
 *
 * Split out from the IO so the rung selection is directly testable; the reading
 * of the chosen files is the caller's job. See the module doc for why the rungs
 * are a precedence chain rather than a union.
 */
export function select_witnesses(
	names: string[],
	ext: string,
): { rung: WitnessRung; witnesses: Witness[] } {
	const matching = (prefix: string): string[] =>
		names.filter((n) => n.startsWith(prefix) && n.endsWith(ext)).sort();

	// Rung 1 — prettier's output on `input` itself.
	const canonical = matching('output_prettier');
	if (canonical.length > 0) {
		return {
			rung: 'output_prettier',
			witnesses: canonical.map((n) => ({ prettier_file: n, source_file: null, label: n })),
		};
	}

	// Rung 2 — every authoring prettier keeps stable and ours maps to `input`.
	// The variant is its OWN source: prettier's fixed point for that authoring.
	const variants = matching('prettier_variant_');
	if (variants.length > 0) {
		return {
			rung: 'prettier_variant',
			witnesses: variants.map((n) => ({ prettier_file: n, source_file: n, label: n })),
		};
	}

	// Rung 3 — prettier's first-pass output, which is what the single-pass
	// corpus comparison sees even though a second pass would move it.
	const intermediates = matching('prettier_intermediate_');
	if (intermediates.length > 0) {
		return {
			rung: 'prettier_intermediate',
			witnesses: intermediates.map((n) => ({ prettier_file: n, source_file: null, label: n })),
		};
	}

	// Rung 4 — a form prettier keeps stable that ours rewrites to a third form.
	// Prettier's output on the fixture's `unformatted_ours_*` authoring stated
	// outright, so it outranks the inference below.
	const divergent = matching('divergent_variant_');
	if (divergent.length > 0) {
		return {
			rung: 'divergent_variant',
			witnesses: divergent.map((n) => ({ prettier_file: n, source_file: null, label: n })),
		};
	}

	// Rung 5 — the N10 inference. Only sound with exactly ONE documented stable
	// form: N6 pins prettier's output off `input`, N10 pins it INTO the fixture's
	// stable-form set, so a single-element set identifies it. Two or more and the
	// pairing would be a guess, so the fixture yields nothing instead.
	// `divergent_variant_*` is deliberately excluded from `variant_` here — the
	// `startsWith` prefix does not match it, and rung 4 already claimed it.
	const stable = matching('variant_');
	const unformatted_ours = matching('unformatted_ours_');
	if (stable.length === 1 && unformatted_ours.length > 0) {
		return {
			rung: 'n10',
			witnesses: [
				{
					prettier_file: stable[0],
					source_file: unformatted_ours[0],
					label: `${stable[0]} (via ${unformatted_ours[0]}, N10)`,
				},
			],
		};
	}

	return { rung: 'none', witnesses: [] };
}

/**
 * The detection cases a fixture pins. Returns `'no_input'` when the directory
 * holds no `input.*` at all.
 */
export async function build_cases(
	fixture_path: string,
	io: FixtureIo = disk_io,
): Promise<DetectionCase[] | 'no_input'> {
	let input_name: string | null = null;
	let input: string | null = null;
	for (const name of INPUT_NAMES) {
		const content = await io.read(fixture_path, name);
		if (content !== null) {
			input_name = name;
			input = content;
			break;
		}
	}
	if (input_name === null || input === null) return 'no_input';

	const language = language_of(input_name);
	const read = async (name: string): Promise<string> => {
		const content = await io.read(fixture_path, name);
		assert(content !== null, `${fixture_path}/${name} vanished between listing and read`);
		return content;
	};

	const { witnesses } = select_witnesses(await io.list(fixture_path), extension_of(input_name));
	const cases: DetectionCase[] = [];
	for (const w of witnesses) {
		cases.push({
			label: w.label,
			source: w.source_file === null ? input : await read(w.source_file),
			ours: input,
			prettier: await read(w.prettier_file),
			language,
		});
	}
	return cases;
}

/** Build the detection context the way `make_context` does. */
function build_context(detection_case: DetectionCase): DetectionContext {
	const diff = diff_lines(detection_case.prettier, detection_case.ours);
	const hunks = extract_hunks(diff);
	const ctx: DetectionContext = {
		source: detection_case.source,
		ours: detection_case.ours,
		prettier: detection_case.prettier,
		diff,
		hunks,
		language: detection_case.language,
	};
	enrich_detection_context(ctx);
	return ctx;
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

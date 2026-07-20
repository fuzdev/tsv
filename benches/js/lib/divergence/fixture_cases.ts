/**
 * Driving divergence detectors against committed fixtures.
 *
 * For a `_prettier_divergence` fixture the committed `input.*` file IS our
 * formatter's output (our formatter is idempotent — input formats to itself).
 * So whenever a committed file pins what *prettier* produces, the pair
 * (ours = input, prettier = that file) is a real divergence a detector must
 * claim — and it can be driven from the committed files alone, with no build
 * and no sidecar.
 *
 * That makes "does any pattern detect this fixture?" a COMPUTABLE fact rather
 * than a hand-maintained one, which is why this lives here rather than inside
 * a test: two consumers ask it. `fixture_coverage_test.ts` asks it per pattern
 * (does this detector still claim the fixtures it lists?), and `validation.ts`
 * asks it per fixture (does ANY detector see this documented divergence?).
 *
 * Reads only.
 */

import { ok as assert } from 'node:assert';
import { diff_lines, extract_hunks } from '../diff.ts';
import { type DetectionContext, enrich_detection_context } from './patterns.ts';
import type { Language } from '../types.ts';

const FIXTURES_ROOT = new URL('../../../../tests/fixtures/', import.meta.url);

/** Candidate input filenames, in resolution order. */
const INPUT_NAMES = ['input.svelte', 'input.svelte.ts', 'input.ts', 'input.css'];

/**
 * Whether a claimed fixture path names a real directory.
 *
 * Any error other than "not found" re-throws — a permission error must never be
 * mistaken for a missing fixture, which would report every listing as broken.
 */
export function fixture_dir_exists(fixture_path: string): boolean {
	try {
		return Deno.statSync(new URL(fixture_path + '/', FIXTURES_ROOT)).isDirectory;
	} catch (err) {
		if (err instanceof Deno.errors.NotFound) return false;
		throw err;
	}
}

/**
 * Read a file, returning null only when it genuinely does not exist. Any other
 * error (notably a permission error) re-throws — a denied read must never be
 * mistaken for a missing fixture file, which would silently hollow out every
 * consumer.
 */
async function read_if_exists(url: URL): Promise<string | null> {
	try {
		return await Deno.readTextFile(url);
	} catch (err) {
		if (err instanceof Deno.errors.NotFound) return null;
		throw err;
	}
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
export interface DetectionCase {
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
 * fixture. The test suite runs `--allow-read` only (deliberately: it must never
 * be able to mutate the tree it audits), so a temp-directory fixture is not an
 * option, and coupling the test to real fixture directories would make it
 * brittle against exactly the renames this machinery exists to catch.
 */
export interface FixtureIo {
	/** Filenames directly inside the fixture directory; empty when it is absent. */
	list: (fixture_path: string) => Promise<string[]>;
	/** File content, or null when the file does not exist. */
	read: (fixture_path: string, name: string) => Promise<string | null>;
}

/** The real filesystem, rooted at `tests/fixtures/`. */
export const disk_io: FixtureIo = {
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
	read: (fixture_path, name) =>
		read_if_exists(new URL(name, new URL(fixture_path + '/', FIXTURES_ROOT))),
};

/** Which rung of the precedence chain supplied a fixture's witnesses. */
export type WitnessRung =
	| 'output_prettier'
	| 'prettier_variant'
	| 'prettier_intermediate'
	| 'divergent_variant'
	| 'n10'
	| 'none';

/** One witness: the file holding prettier's output, and the authoring it came from. */
export interface Witness {
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
 * of the chosen files is the caller's job.
 *
 * The rungs are a PRECEDENCE chain, not a union — each is used only when the
 * rung above it is absent:
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
export function build_context(detection_case: DetectionCase): DetectionContext {
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

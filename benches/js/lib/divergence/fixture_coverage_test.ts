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
 * to itself), and `output_prettier.*` is prettier's output. So we can drive the
 * detector against the committed files with no build/sidecar:
 *   source = input, ours = input, prettier = output_prettier.
 *
 * Fixtures whose prettier form is captured as `prettier_variant_*` (no single
 * `output_prettier.*`) are skipped with a warning — there is no single prettier
 * output to diff against, so the input-is-ours insight does not apply.
 *
 * Runs read-only (`--allow-read`).
 */

import { ok as assert } from 'node:assert';
import { diff_lines, extract_hunks } from '../diff.ts';
import { type DetectionContext, enrich_detection_context, PATTERNS } from './patterns.ts';
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

/** Corresponding `output_prettier.*` name for a given input filename. */
function prettier_name_for(input_name: string): string {
	if (input_name.endsWith('.svelte.ts')) return 'output_prettier.svelte.ts';
	if (input_name.endsWith('.svelte')) return 'output_prettier.svelte';
	if (input_name.endsWith('.ts')) return 'output_prettier.ts';
	if (input_name.endsWith('.css')) return 'output_prettier.css';
	return 'output_prettier.svelte';
}

interface FixtureFiles {
	input_name: string;
	input_content: string;
	prettier_content: string;
}

/** Load a fixture's input + output_prettier, or null if either is absent. */
async function load_fixture(fixture_path: string): Promise<FixtureFiles | 'no_input' | null> {
	const dir = new URL(fixture_path + '/', FIXTURES_ROOT);

	let input_name: string | null = null;
	let input_content: string | null = null;
	for (const name of INPUT_NAMES) {
		const content = await read_if_exists(new URL(name, dir));
		if (content !== null) {
			input_name = name;
			input_content = content;
			break;
		}
	}
	if (input_name === null || input_content === null) return 'no_input';

	const prettier_content = await read_if_exists(new URL(prettier_name_for(input_name), dir));
	if (prettier_content === null) return null; // prettier_variant_* only — no single output

	return { input_name, input_content, prettier_content };
}

/** Build the detection context the way `make_context` does, with source = input. */
function build_context(files: FixtureFiles): DetectionContext {
	const language = language_of(files.input_name);
	const diff = diff_lines(files.prettier_content, files.input_content);
	const hunks = extract_hunks(diff);
	const ctx: DetectionContext = {
		source: files.input_content,
		ours: files.input_content,
		prettier: files.prettier_content,
		diff,
		hunks,
		language,
	};
	enrich_detection_context(ctx);
	return ctx;
}

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
		for (const fixture_path of pattern.fixtures) {
			const loaded = await load_fixture(fixture_path);

			if (loaded === 'no_input') {
				console.warn(
					`[fixture coverage] ${pattern.id}: no input.* found for ${fixture_path} — skipping`,
				);
				continue;
			}
			if (loaded === null) {
				console.warn(
					`[fixture coverage] ${pattern.id}: no output_prettier.* for ${fixture_path} ` +
						`(prettier_variant-only fixture) — skipping`,
				);
				continue;
			}

			const ctx = build_context(loaded);
			const match = pattern.detect(ctx);
			const claims_hunk = match !== null && match.hunk_indices.length > 0;

			const key = `${pattern.id}\t${fixture_path}`;
			if (PRE_EXISTING_DRIFT.has(key)) {
				// TODO: pre-existing drift — pattern does not detect claimed fixture.
				// Soft warning keeps the suite green while the drift stays visible.
				if (!claims_hunk) {
					console.warn(
						`[fixture coverage] PRE-EXISTING DRIFT: ${pattern.id} does not detect ` +
							`its claimed fixture ${fixture_path}`,
					);
				} else {
					// The drift was fixed elsewhere — flag so the entry can be removed.
					console.warn(
						`[fixture coverage] ${pattern.id} now DETECTS ${fixture_path}; ` +
							`remove it from PRE_EXISTING_DRIFT`,
					);
				}
				continue;
			}

			assert(
				claims_hunk,
				`${pattern.id} does not claim any hunk in its own fixture ${fixture_path} ` +
					`(match=${match === null ? 'null' : JSON.stringify(match.hunk_indices)})`,
			);
		}
	});
}

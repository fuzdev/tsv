/**
 * Shared vocabulary for the two consumers of the `../typescript` test corpus —
 * the `conformance:ts-repo` gate (`diagnostics/ts_repo_compare.ts`) and the
 * tsc-corpus harvest (`harvest_ts_repo.ts`).
 *
 * They ask different questions of the same tree, over deliberately different roots
 * (the gate grades tsv per file over the WHOLE `tests/cases`; the harvest filters
 * `conformance` + `compiler` into a bench corpus — see `DEFAULT_ROOT` in
 * `diagnostics/ts_repo_compare.ts` for why the gate takes the superset), but they
 * must agree on **what the corpus IS** and on **what
 * tsc's baselines SAY** — otherwise the bench corpus and the gate silently grade
 * different populations against different oracles. Both answers live here rather
 * than as parallel copies:
 *
 * - **Discovery** — which files are parse units at all (`discover_ts_cases`,
 *   `is_multi_file_test`).
 * - **The baseline key rule + the grammar-error test** — how a `.errors.txt`
 *   filename maps to a test name, and which diagnostic codes mean *tsc's parser*
 *   rejected (`baseline_test_key`, `has_grammar_error`). The two callers index the
 *   baselines into different shapes (a `Set` of rejecting tests vs a `Map` of every
 *   variant), which is fine — the rule under both is the same one.
 */

import { readdir } from 'node:fs/promises';
import { join } from 'node:path';

/** The official TypeScript checkout — baselines and test cases live at fixed paths under it. */
export const TS_REPO = '../typescript';

/** Where the TS test harness writes its `<name>[(setting=value)].errors.txt` baselines. */
export const TS_BASELINE_DIR = `${TS_REPO}/tests/baselines/reference`;

/**
 * Files `discover_ts_cases` filtered out, tallied so a caller can report them.
 *
 * Both callers scope themselves to single-file `.ts`, and both must be able to say
 * how much they dropped getting there — a corpus that reports only what it graded
 * reads as having covered everything.
 */
export interface TsCaseSkips {
	/** `.d.ts` declaration files — not parse tests for either consumer. */
	declaration: number;
	/** `.tsx` — JSX grammar, out of scope for tsv. */
	tsx: number;
}

/** A zero-valued {@link TsCaseSkips} tally, for a caller to pass in and then report. */
export function empty_ts_case_skips(): TsCaseSkips {
	return { declaration: 0, tsx: 0 };
}

/**
 * Every `.ts` test case under `dir`, recursively — the corpus definition both
 * consumers share. `.d.ts` and `.tsx` are filtered here and counted into `skips`
 * (never silently), `node_modules` is pruned, and an unreadable directory is
 * reported and skipped rather than aborting the walk.
 */
export async function* discover_ts_cases(dir: string, skips: TsCaseSkips): AsyncGenerator<string> {
	let entries;
	try {
		entries = await readdir(dir, { withFileTypes: true });
	} catch (e) {
		console.error(`Cannot read ${dir}: ${e instanceof Error ? e.message : e}`);
		return;
	}
	for (const entry of entries) {
		const full = join(dir, entry.name);
		if (entry.isDirectory()) {
			if (entry.name !== 'node_modules') yield* discover_ts_cases(full, skips);
		} else if (entry.name.endsWith('.d.ts')) {
			skips.declaration++;
		} else if (entry.name.endsWith('.tsx')) {
			skips.tsx++;
		} else if (entry.name.endsWith('.ts')) {
			yield full;
		}
	}
}

/**
 * Whether a test case is a multi-file test — several virtual modules concatenated
 * behind `// @filename:` directives, which is not one parse unit. Both consumers
 * skip these; feeding one to a parser as a single source is meaningless.
 */
export function is_multi_file_test(content: string): boolean {
	return /\/\/\s*@[Ff]ilename:/.test(content);
}

/**
 * The test name a `*.errors.txt` baseline belongs to — the filename minus the
 * trailing `(setting=value)` group and the extension.
 *
 * A test compiled under multiple settings (`// @target: es5, es2015`, `@module`, …)
 * writes per-variant baselines (`<name>(target=es5).errors.txt`) rather than a plain
 * `<name>.errors.txt`, so a lookup keyed on the bare name MISSES the suffixed ones
 * and mis-reads a tsc grammar rejection as a clean compile (e.g. `parserAccessors5`
 * → TS1183 lives in `parserAccessors5(target=es5).errors.txt`). Index by this key so
 * one lookup gathers every variant.
 */
export function baseline_test_key(baseline_name: string): string {
	return baseline_name.replace(/\.errors\.txt$/, '').replace(/\(.*\)$/, '');
}

/**
 * Whether a baseline's text carries a `TS1xxx` diagnostic — tsc's **parser**
 * rejected the input. `TS2xxx`+ are semantic (checker-side), so tsc's parser
 * accepted. This is the validity oracle both consumers read; grammar errors are
 * target-independent, so a code in ANY variant is a rejection.
 */
export function has_grammar_error(baseline_text: string): boolean {
	return /error TS1\d{3}:/.test(baseline_text);
}

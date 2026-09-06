/**
 * Shared vocabulary for the two consumers of the `../typescript` test corpus —
 * the `conformance:ts-repo` gate (`diagnostics/ts_repo_compare.ts`) and the
 * tsc-corpus harvest (`harvest_ts_repo.ts`).
 *
 * They ask different questions of the same tree, so they scope themselves
 * differently along exactly THREE declared axes — the **root** (the gate grades tsv
 * per file over the WHOLE `tests/cases`; the harvest filters `conformance` +
 * `compiler` into a bench corpus), the **declaration policy** ({@link
 * DeclarationPolicy}: the gate admits `.d.ts`, the harvest skips it), and the
 * **multi-file tests** (the gate splits them into units through
 * {@link split_test_units}; the harvest skips them, since a unit has no path of its
 * own for the bench to hand a tool). Each axis is argued where the consumer sets it —
 * `DEFAULT_ROOT` and `DECLARATIONS` in `diagnostics/ts_repo_compare.ts`,
 * `DECLARATIONS` in `harvest_ts_repo.ts`. Every OTHER answer must be the same on both
 * sides: **what a parse unit is** and **what tsc's baselines SAY** — otherwise the
 * bench corpus and the gate silently grade different populations against different
 * oracles. Those live here rather than as parallel copies:
 *
 * - **Discovery** — which files are parse units at all (`discover_ts_cases`,
 *   `is_multi_file_test`), the unit split of a multi-file test and a unit's
 *   language (`split_test_units`, `test_unit_language`), and the one knob discovery takes.
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
	/** `.d.ts` declaration files, dropped under a `'skip'` {@link DeclarationPolicy} (0 under `'include'`). */
	declaration: number;
	/** `.tsx` — JSX grammar, out of scope for tsv. */
	tsx: number;
}

/** A zero-valued {@link TsCaseSkips} tally, for a caller to pass in and then report. */
export function empty_ts_case_skips(): TsCaseSkips {
	return { declaration: 0, tsx: 0 };
}

/**
 * Whether a consumer admits `.d.ts` declaration files as parse units.
 *
 * A `.d.ts` is ordinary TypeScript source to tsv — no declaration mode exists, the
 * product formats one like any other `.ts`, and the live-code corpus admits them
 * for exactly that reason (`lib/corpus.ts`). So the two consumers here differ not
 * on whether the files are real, but on whether each one's *oracle* can grade
 * them, which is a property of the consumer, not of the file. Required rather than
 * defaulted so each side has to state its answer.
 */
export type DeclarationPolicy = 'include' | 'skip';

/**
 * Every `.ts` test case under `dir`, recursively — the parse-unit rule both
 * consumers share. `.tsx` is always filtered, `.d.ts` per `declarations`, both
 * counted into `skips` (never silently); `node_modules` is pruned, and an
 * unreadable directory is reported and skipped rather than aborting the walk.
 */
export async function* discover_ts_cases(
	dir: string,
	skips: TsCaseSkips,
	declarations: DeclarationPolicy
): AsyncGenerator<string> {
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
			if (entry.name !== 'node_modules') yield* discover_ts_cases(full, skips, declarations);
		} else if (entry.name.endsWith('.d.ts')) {
			if (declarations === 'skip') skips.declaration++;
			else yield full;
		} else if (entry.name.endsWith('.tsx')) {
			skips.tsx++;
		} else if (entry.name.endsWith('.ts')) {
			yield full;
		}
	}
}

/**
 * The `// @filename:` directive that opens a virtual file inside a multi-file test,
 * as tsc's own harness reads it (`optionRegex` in `src/harness/harnessIO.ts`,
 * consumed per line by `makeUnitsFromTest`). One source pattern, so the
 * multi-file TEST (`is_multi_file_test`) and the unit SPLIT (`split_test_units`)
 * cannot disagree about which lines are directives. The harness rule differs from
 * the naive reading on both axes:
 *
 * - **The directive name is case-INSENSITIVE** — the harness lowercases it before
 *   comparing (`metaDataName !== "filename"`), so `@fileName` and `@FILENAME` split
 *   units exactly like `@filename`.
 * - **The `//` is ANCHORED to line start, and is exactly two slashes** — the harness
 *   regex is `/^\/{2}\s*@(\w+)\s*:/`. An unanchored match also fires inside a
 *   fourslash `////` body, where the text is a virtual file's *content*
 *   (`fourslashImpl.ts` tests `line.substr(0, 4) === "////"` first, before it ever
 *   looks for a directive), so a commented-out `//// // @Filename:` would split a
 *   file that is one parse unit under both harnesses.
 *
 * Interior whitespace stays horizontal-only: the harness's `\s*` can cross a newline
 * when run over whole content, but `makeUnitsFromTest` applies it per line, and the
 * cross-line reading matches no file in the corpus.
 */
const FILENAME_DIRECTIVE = String.raw`^\/\/[^\S\r\n]*@filename[^\S\r\n]*:`;
/** The directive anywhere in a document. */
const FILENAME_DIRECTIVE_ANYWHERE = new RegExp(FILENAME_DIRECTIVE, 'im');
/** The directive as one whole line (no terminator), capturing the name it opens. */
const FILENAME_DIRECTIVE_LINE = new RegExp(`${FILENAME_DIRECTIVE}(.*)$`, 'i');

/**
 * Whether a test case is a multi-file test — several virtual modules concatenated
 * behind `// @filename:` directives, which is not one parse unit. Feeding one to a
 * parser as a single source is meaningless: the harvest skips these, the gate
 * grades their units through {@link split_test_units}.
 */
export function is_multi_file_test(content: string): boolean {
	return FILENAME_DIRECTIVE_ANYWHERE.test(content);
}

/** One virtual file of a multi-file test: the directive's name and the lines under it. */
export interface TestUnit {
	/** The path the directive names, trimmed — `/a.ts`, `node_modules/x/index.d.ts`, `b.mts`. */
	name: string;
	/** Everything between this directive line and the next (or EOF), line terminators kept. */
	content: string;
}

/**
 * A multi-file test's virtual files, split where tsc's harness splits them
 * (`makeUnitsFromTest`): each `// @filename:` line opens a unit that runs to the
 * next directive or EOF. Two deliberate simplifications against the harness:
 *
 * - **Lines before the first directive are dropped.** The harness allows only
 *   trivia there (it throws on anything else — the global `// @option:` lines and
 *   comments), and discards them the same way.
 * - **Other `// @option:` lines stay in the unit.** The harness strips every
 *   directive from the unit's content; a parser reads them as comments, so leaving
 *   them in keeps the unit's bytes the author's, which is what the single-file path
 *   grades too. The one shape this changes — a `#!` shebang under a directive, which
 *   the harness's strip would move to byte 0 — is caught by the gate's tsc-parser
 *   check on the same raw bytes, never mis-read as a tsv gap.
 *
 * A unit's language is the caller's question ({@link test_unit_language}): the
 * split returns every unit, `package.json` and `.tsx` included, so a caller can
 * count what it declines.
 */
export function split_test_units(content: string): TestUnit[] {
	const units: TestUnit[] = [];
	let current: TestUnit | null = null;
	for (const line of content.split(/(?<=\n)/)) {
		const match = FILENAME_DIRECTIVE_LINE.exec(line.replace(/\r?\n$/, ''));
		if (match) {
			if (current) units.push(current);
			current = { name: match[1]!.trim(), content: '' };
		} else if (current) {
			current.content += line;
		}
	}
	if (current) units.push(current);
	return units;
}

/**
 * What a virtual file's name says it holds — the extension class a consumer keys
 * its scope on. `ts` is the JS/TS family tsv formats as TypeScript minus `.tsx`
 * (`.ts`, `.mts`, `.cts`, and the `.d.*` spellings of each); `tsx` is JSX grammar,
 * out of tsv's scope; `js` is JavaScript, which tsc parses under its own JS rules
 * (JSDoc types, no TS syntax) and so is not graded by a tsc-baseline oracle keyed
 * on TS; `other` is everything a compile reads without parsing as code —
 * `package.json`, `tsconfig.json`, `.css`, `.md`.
 */
export function test_unit_language(name: string): 'ts' | 'tsx' | 'js' | 'other' {
	const lower = name.toLowerCase();
	if (lower.endsWith('.tsx')) return 'tsx';
	if (lower.endsWith('.ts') || lower.endsWith('.mts') || lower.endsWith('.cts')) return 'ts';
	if (
		lower.endsWith('.js') ||
		lower.endsWith('.jsx') ||
		lower.endsWith('.mjs') ||
		lower.endsWith('.cjs')
	) {
		return 'js';
	}
	return 'other';
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
 *
 * A `.d.ts` case needs no special handling: the harness names its baseline
 * `<name>.d.errors.txt`, which strips to `<name>.d` — exactly what a caller stripping
 * the trailing `.ts` off `<name>.d.ts` asks for. The two halves of the rule meet on
 * their own, so declaration cases key correctly wherever the walk admits them.
 */
export function baseline_test_key(baseline_name: string): string {
	return baseline_name.replace(/\.errors\.txt$/, '').replace(/\(.*\)$/, '');
}

/**
 * The distinct `TS1xxx` codes a baseline's text carries — tsc's **grammar**
 * diagnostics. `TS2xxx`+ are semantic, so a file with none of these is one tsc's
 * grammar accepted. This is the validity oracle both consumers read; grammar
 * errors are target-independent, so a code in ANY variant counts.
 *
 * ⚠️ A `TS1xxx` code is NOT proof that tsc's *parser* rejected: the range spans the
 * parser and the checker's `checkGrammar*` family (TS1036 ambient statements,
 * TS1040 ambient `async`, TS1206 decorator placement are all checker-raised). A
 * consumer that needs the parser's own verdict runs it (`lib/tsc.ts`), which is why
 * the codes come back rather than a boolean — the gate histograms them.
 */
export function grammar_error_codes(baseline_text: string): string[] {
	return [...new Set([...baseline_text.matchAll(/error (TS1\d{3}):/g)].map((m) => m[1]!))];
}

/** Whether a baseline's text carries any `TS1xxx` diagnostic — see {@link grammar_error_codes}. */
export function has_grammar_error(baseline_text: string): boolean {
	return grammar_error_codes(baseline_text).length > 0;
}

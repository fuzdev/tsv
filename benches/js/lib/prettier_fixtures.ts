/**
 * What Prettier's own format suites say about their fixtures — the filter the
 * `prettier_fixture` corpus entries apply on top of the shared exclusions.
 *
 * The suites under `../prettier/tests/format/{js,typescript,css}` are formatter
 * fixtures, not a validity corpus: beside plain code they carry Prettier's
 * harness markers, front matter, Babel-only proposal syntax, and inputs kept
 * precisely because a parser must reject them. Grading parse coverage over the
 * raw walk counts every one of those against every parser at once, so the JS
 * suite read as 77–85% for the whole field with the real gaps buried in the
 * deltas. Prettier already knows which is which, in two tool-neutral places this
 * module reads rather than re-deciding:
 *
 * - **Content markers.** Range tests embed `<<<PRETTIER_RANGE_START>>>` /
 *   `<<<PRETTIER_RANGE_END>>>` and cursor tests `<|>` in the source itself
 *   (`tests/config/format-test/constants.js`); the runner strips them before
 *   formatting, so the bytes on disk are valid for no parser. The CSS and HTML
 *   parsers lift a front-matter block off the source the same way (the JS and
 *   TypeScript parsers never do, so those suites take the markers alone), and
 *   whether there IS one is Prettier's own `getFrontMatter` rule (an opening
 *   fence AND a closing one — a file that only opens with `---` goes to the
 *   language parser whole, and its spec's verdict applies). The shared
 *   `cursor/` and `front-matter/` directory exclusions catch the dirs named for
 *   the feature; this catches the files by what they contain, wherever they sit
 *   (`range/`, `eol/cursor-1.js`, `css/yaml/`).
 * - **Per-directory verdicts.** Each fixture directory's `format.test.js` names
 *   the parsers it runs (`runFormatTest(import.meta, ["babel"], …)`) and, in its
 *   `errors` option, which of them must THROW on which files — Prettier's own
 *   record of "this input is not valid for that grammar". The runner also adds
 *   implicit verify parsers (`get-parsers.js`: `babel` in a JS suite pulls in
 *   acorn, espree, meriyah, oxc; `typescript` pulls in babel-ts, oxc-ts), which
 *   is why a `["babel"]` directory can list `acorn: true`. A fixture is kept
 *   when at least one SPEC-GRAMMAR parser ({@link PRETTIER_SPEC_PARSERS}) is
 *   run on it without a declared error — Prettier expects a parser of the
 *   language itself to accept it. A fixture every spec-grammar parser is
 *   expected to reject is a proposal or an error case: dropped, and a declared
 *   error for a spec-grammar parser the directory does not even run counts the
 *   same way (`typescript/definite/` runs `babel-ts` alone and records that
 *   `typescript` throws — Prettier's word on the file, whether or not its
 *   runner would consult it). A directory that runs no spec-grammar parser and
 *   declares nothing gives no verdict, and its files stay: the one such case,
 *   `typescript/typescript-babel-only/`, is named for what its spec never
 *   states. An `_errors_/` directory is what the runner makes it: no implicit
 *   parsers, and every parser throws unless the spec says otherwise — so its
 *   files drop here on their own, whether or not a path exclusion got there
 *   first. The spec file itself is no fixture (Prettier's `getFiles` skips it)
 *   and is dropped too: it is plain harness JS every parser reads — 459 files
 *   across the three suites, a fifth of their raw walk, and in the CSS and
 *   TypeScript suites the only JS-language files at all, graded under a
 *   language those suites are not about — lifting every rate alike and
 *   measuring nothing.
 *
 * Both rules are Prettier's, applied per file, so they cannot bias the corpus
 * toward any parser under test — the posture of every other harvest here
 * (test262's front matter, tsc's baselines, svelte/compiler's rejects).
 *
 * They are Prettier's word, not the spec's, and the two differ at the edges.
 * Prettier verifies with lenient options (acorn's `allowReturnOutsideFunction` /
 * `allowSuperOutsideMethod` / `allowReserved`, babel's
 * `allowNewTargetOutsideFunction`, …), and `oxc` accepts some early errors its
 * peers raise, so a fixture that is no strict ECMAScript can keep a clean
 * spec-grammar verdict. Three such JS fixtures are known at the pinned checkout,
 * each rejected by acorn, acorn-typescript and at least one of oxc / tsc:
 * `return-outside-function/return-outside-function.js` (a top-level `return`),
 * `new-target/outside-functions.js` (a top-level `new.target`), and
 * `strings/non-octal-eight-and-nine.js` (a `"\8"` in a strict function body,
 * which its spec records acorn, espree and meriyah throwing on and only oxc
 * accepts). They stay rather than being special-cased: the filter reads
 * Prettier's verdicts, it does not re-grade them.
 *
 * A third exclusion is SCOPE rather than validity and lives in a harvest cache
 * (`diagnostics/prettier_jsx_harvest.ts`): the `.js` fixtures Prettier's own
 * babel parser reads as JSX ({@link ast_has_jsx}). Prettier's spec parsers all
 * take JSX in a `.js` file, so the verdicts above keep them; the coverage
 * surface runs every parser in TypeScript mode, where JSX is rejected by
 * all of them alike, so those files carried no signal and are dropped the way
 * the `jsx/` suite and the compiler's `.tsx` cases already are.
 *
 * The spec files are evaluated, not pattern-matched: they are small ES modules
 * that build their arguments with `const` lists, spreads, and helpers, which a
 * regex would misread silently. Evaluation binds `runFormatTest` to a recorder,
 * stubs the handful of imports and test globals the specs touch, and fails
 * LOUDLY on anything else — a spec this cannot read must not quietly grade as
 * "no verdict".
 */

import { readFile } from 'node:fs/promises';
import { basename, dirname, join, sep } from 'node:path';

/** The cursor placeholder Prettier's `formatWithCursor` fixtures embed. */
const PRETTIER_CURSOR_MARKER = '<|>';

/** The range placeholders Prettier's range-format fixtures embed. */
export const PRETTIER_RANGE_MARKERS = [
	'<<<PRETTIER_RANGE_START>>>',
	'<<<PRETTIER_RANGE_END>>>'
] as const;

/**
 * The parsers whose acceptance is a verdict about the language's grammar rather
 * than Babel's plugin set: the ECMAScript/TypeScript parsers Prettier verifies
 * against, and postcss for the CSS suite. `babel*`, `flow`, and `hermes` parse
 * proposals and Flow, so their acceptance says nothing about validity here.
 * The line is Prettier's, not ecma262's: `oxc` and `meriyah` take some stage-3
 * syntax (decorators, `import defer`), so a fixture only they accept stays.
 */
const PRETTIER_SPEC_PARSERS: ReadonlySet<string> = new Set([
	'typescript',
	'acorn',
	'espree',
	'meriyah',
	'oxc',
	'oxc-ts',
	'css'
]);

/** The file every fixture directory's verdicts live in. */
const PRETTIER_SPEC_FILENAME = 'format.test.js';

/**
 * Prettier's `errors` option: `true` for "every parser throws on every file",
 * else per parser `true` or the list of filenames it throws on.
 */
export type PrettierErrors = true | Record<string, true | string[]>;

/** One `runFormatTest(...)` call a spec file makes. */
export interface PrettierFormatTestCall {
	parsers: string[];
	errors: PrettierErrors | undefined;
}

/**
 * True when an ESTree/Babel AST holds a JSX node anywhere — the reading Prettier's
 * babel parser gives a `.js` fixture that the coverage surface must drop as
 * out-of-scope syntax. Walks every object-valued property, so it is shape-agnostic
 * (`JSXElement` under `expression`, a `JSXFragment` in an array of arguments, …);
 * the `JSX` prefix is Babel's and ESTree's own naming for the whole family.
 */
export const ast_has_jsx = (node: unknown): boolean => {
	const seen = new Set<object>();
	const visit = (value: unknown): boolean => {
		if (value === null || typeof value !== 'object' || seen.has(value)) return false;
		seen.add(value);
		if (Array.isArray(value)) return value.some(visit);
		const { type } = value as { type?: unknown };
		if (typeof type === 'string' && type.startsWith('JSX')) return true;
		return Object.values(value).some(visit);
	};
	return visit(node);
};

/** True when the source carries a range or cursor placeholder — invalid for every parser. */
export const prettier_fixture_has_markers = (content: string): boolean =>
	content.includes(PRETTIER_CURSOR_MARKER) ||
	PRETTIER_RANGE_MARKERS.some((marker) => content.includes(marker));

/** The front-matter fences Prettier's `getFrontMatter` opens on. */
const FRONT_MATTER_DELIMITERS = new Set(['---', '+++']);

/**
 * True when Prettier lifts a front-matter block off the source — its
 * `src/main/front-matter/parse.js` mirrored: a `---` or `+++` fence at byte 0
 * (a BOM ahead of it is stripped, and CR / CRLF line ends folded to LF, before
 * Prettier looks), an optional language after the fence, and a CLOSING fence at
 * the head of a later line (`...` closes YAML too). A lone opening fence is not
 * front matter: Prettier hands that file to the language parser whole
 * (`css/yaml/malformed.css`, which postcss accepts), so the spec's verdict
 * applies to it.
 */
export const prettier_fixture_has_front_matter = (content: string): boolean => {
	const text = (content.startsWith('\uFEFF') ? content.slice(1) : content).replace(/\r\n?/g, '\n');
	const delimiter = text.slice(0, 3);
	if (!FRONT_MATTER_DELIMITERS.has(delimiter)) return false;
	const first_break = text.indexOf('\n', 3);
	if (first_break === -1) return false;
	if (text.indexOf(`\n${delimiter}`, first_break) !== -1) return true;
	const language = text.slice(3, first_break).trim() || (delimiter === '+++' ? 'toml' : 'yaml');
	return delimiter === '---' && language === 'yaml' && text.indexOf('\n...', first_break) !== -1;
};

/**
 * The content-side filter for a suite whose parser lifts front matter (CSS,
 * HTML): markers and front matter. The JS and TypeScript suites take
 * {@link prettier_fixture_has_markers} alone — their parsers read a leading
 * fence as source.
 */
export const prettier_fixture_content_skip = (content: string): boolean =>
	prettier_fixture_has_markers(content) || prettier_fixture_has_front_matter(content);

/**
 * A suite under `tests/format/` the verdict filter is wired for — Prettier keys
 * its implicit verify parsers on it. `html` is not one: its parser is no
 * spec-grammar parser, so that suite takes the content filter alone.
 */
export type PrettierSuite = 'typescript' | 'js' | 'css';

/**
 * The parsers Prettier actually runs for a call — the explicit list plus the
 * implicit verify parsers `tests/config/format-test/get-parsers.js` adds, none
 * of them in an `_errors_/` directory (`error_test`). The runner's
 * `disabledTests` (`failed-format-tests.js`: a parser it skips at a named file
 * or directory) is not mirrored, and the omission is not neutral in principle:
 * a skipped parser is one Prettier does NOT run, so reading it as run without
 * a declared error can keep a file no run spec-grammar parser accepts. It is
 * neutral at the pinned checkout, checked entry by entry: every spec-grammar
 * skip (`acorn`/`espree` at one `explicit-resource-management` file, `espree`
 * over `comments-closure-typecast/` and its `no-semi/`, `meriyah` at one
 * `decorators` file, `oxc-ts` over `typescript-only/`, `typescript` at two
 * `long-module-name` files) sits where another spec-grammar parser runs
 * with no declared error, so no verdict moves. A Prettier bump re-asks that.
 */
export const prettier_parsers_run = (
	suite: PrettierSuite,
	explicit: string[],
	error_test = false
): string[] => {
	const all = [...explicit];
	if (error_test) return all;
	const add = (...parsers: string[]) => {
		for (const parser of parsers) if (!all.includes(parser)) all.push(parser);
	};
	if (explicit.includes('babel') && suite === 'js') {
		add('acorn', 'espree', 'meriyah', 'oxc');
	}
	if (explicit.includes('typescript')) add('babel-ts', 'oxc-ts');
	if (explicit.includes('flow')) add('hermes');
	if (explicit.includes('babel')) add('__babel_estree');
	return all;
};

/** `shouldThrowOnFormat` from Prettier's `tests/config/format-test/utilities.js`. */
export const prettier_expects_throw = (
	filename: string,
	parser: string,
	errors: PrettierErrors | undefined
): boolean => {
	if (errors === true) return true;
	const files = errors?.[parser];
	return files === true || (Array.isArray(files) && files.includes(filename));
};

/**
 * Prettier's verdict on one fixture: `true` when some spec-grammar parser is run
 * on it with no declared error, `false` when every spec-grammar parser run is
 * expected to throw — or when the only spec-grammar verdicts are declared
 * errors for parsers the call never runs — and `null` when no spec-grammar
 * parser is run or named at all. In an `_errors_/` directory (`error_test`)
 * the runner's defaults apply: no implicit parsers, `errors: true` unless the
 * spec says otherwise.
 */
export const prettier_fixture_expected_valid = (
	filename: string,
	suite: PrettierSuite,
	calls: PrettierFormatTestCall[],
	error_test = false
): boolean | null => {
	let verdict: boolean | null = null;
	for (const call of calls) {
		const errors = error_test ? (call.errors ?? true) : call.errors;
		const run = prettier_parsers_run(suite, call.parsers, error_test);
		// a declared error counts even for a parser the call never runs
		const declared = typeof errors === 'object' ? Object.keys(errors) : [];
		for (const parser of new Set([...run, ...declared])) {
			if (!PRETTIER_SPEC_PARSERS.has(parser)) continue;
			if (prettier_expects_throw(filename, parser, errors)) verdict = false;
			else if (run.includes(parser)) return true;
		}
	}
	return verdict;
};

/**
 * Prettier's `isErrorTest`: a directory at or below one named `_errors_`, where
 * the runner adds no implicit parsers and expects every parser to throw.
 */
export const prettier_error_test_dir = (dir: string): boolean =>
	`${sep}${dir}${sep}`.includes(`${sep}_errors_${sep}`);

// Prettier's spec files import a few helpers and call the Jest globals; each is
// bound to this stub, which is callable, taggable, coerces to an empty string,
// and has every property, so the module body runs to its `runFormatTest` calls
// without the real thing. Only those calls are read — a stub reaching an argument
// is fine (the `directives` spec's `snippets` are `outdent` strings concatenated
// with `"\n"`), and a stub standing in for a PARSER LIST or an `errors` entry is
// caught below, where the recorder requires real strings. It refuses every way a
// stub could stand in for a list's CONTENTS and read as fewer entries: iterating
// it (`[...imported, "babel"]`, `errors: { acorn: [...imported] }`), concatenating
// it (`["babel"].concat(imported)`, which spreads a value claiming
// `Symbol.isConcatSpreadable`), and listing its keys (`errors: { ...imported }`)
// all throw, so the spec fails to evaluate, loudly. The one reading no trap can
// see is truthiness — `imported ? a : b` always takes `a` — so a spec that
// branches on an import is read on that branch alone.
const refuse = (what: string) => () => {
	throw new Error(`${what} an imported value (a stubbed import standing in for a list)`);
};
const STUB: unknown = new Proxy(function stub() {}, {
	get: (_target, prop) => {
		if (prop === Symbol.iterator) return refuse('iterated');
		if (prop === Symbol.isConcatSpreadable) return refuse('concatenated')();
		if (prop === Symbol.toPrimitive || prop === 'toString' || prop === 'valueOf') return () => '';
		return STUB;
	},
	ownKeys: refuse('spread'),
	apply: () => STUB
});

/**
 * Whether a spec's `errors` option is one `shouldThrowOnFormat` can read: `true`,
 * a record of parser → `true` or filename list, or an array, which it indexes by
 * parser name and so reads as naming no parser.
 */
const is_readable_errors = (errors: unknown): errors is PrettierErrors | unknown[] =>
	errors === true ||
	Array.isArray(errors) ||
	(typeof errors === 'object' &&
		errors !== null &&
		Object.values(errors).every(
			(files) =>
				files === true || (Array.isArray(files) && files.every((file) => typeof file === 'string'))
		));

const IMPORT_RE =
	/^import\s+(?:([\w$]+)\s*,?\s*)?(?:\{([^}]*)\})?\s*(?:from\s*)?["'][^"']+["'];?[ \t]*$/gm;
const JEST_GLOBALS = ['test', 'it', 'beforeAll', 'afterAll', 'expect'];

/**
 * Jest's `describe`, which runs its body at collection time — so a
 * `runFormatTest` call inside one is a call the spec makes, and is recorded. The
 * other globals take the stub: a test body runs no `runFormatTest` (Jest forbids
 * declaring tests inside one).
 */
const describe = (_name: unknown, body?: unknown): void => {
	if (typeof body === 'function') body();
};

/**
 * Evaluate a `format.test.js` and return its `runFormatTest` calls. Throws,
 * naming the spec, on a shape it cannot bind — never a silent empty list.
 */
export const evaluate_prettier_spec = (
	source: string,
	spec_path: string
): PrettierFormatTestCall[] => {
	const bound = new Set<string>(JEST_GLOBALS);
	const body = source
		.replace(IMPORT_RE, (_m, default_name: string | undefined, named: string | undefined) => {
			if (default_name) bound.add(default_name);
			for (const spec of named?.split(',') ?? []) {
				const local = spec
					.trim()
					.split(/\s+as\s+/)
					.pop()
					?.trim();
				if (local) bound.add(local);
			}
			return '';
		})
		.replaceAll('import.meta', '__import_meta');
	const calls: PrettierFormatTestCall[] = [];
	const run_format_test = (_fixtures: unknown, parsers: unknown, options?: unknown) => {
		if (!Array.isArray(parsers) || !parsers.every((p) => typeof p === 'string')) {
			throw new Error(`prettier spec passed a non-string parser list: ${spec_path}`);
		}
		const errors = (options as { errors?: unknown } | undefined)?.errors;
		if (errors !== undefined && !is_readable_errors(errors)) {
			throw new Error(`prettier spec passed an unreadable \`errors\`: ${spec_path}`);
		}
		// `shouldThrowOnFormat` indexes `errors` by parser name, so an ARRAY
		// (`js/in/`'s spec lists a filename there) declares nothing for any parser —
		// recorded as the empty record it reads as, never as an absent option, which
		// an `_errors_/` directory would default to `true`.
		calls.push({ parsers, errors: Array.isArray(errors) ? {} : errors });
	};
	const names = [...bound];
	// An import or export the rewrite above left in place is a SyntaxError in a
	// function body, so `new Function` refuses it here — a pre-check over the text
	// would also match a snippet line inside a template literal that starts with
	// `import`/`export` (`_errors_/decorators/`), refusing a spec it can read.
	try {
		const fn = new Function('runFormatTest', '__import_meta', 'describe', ...names, body);
		fn(
			run_format_test,
			{ url: `file://${spec_path}`, dirname: dirname(spec_path), filename: spec_path },
			describe,
			...names.map(() => STUB)
		);
	} catch (e) {
		throw new Error(`prettier spec failed to evaluate: ${spec_path}: ${e}`, { cause: e });
	}
	// A spec that declares its tests some way this cannot see (a helper that wraps
	// `runFormatTest`, a Jest global other than `describe` running it) records
	// nothing, and nothing would read as "no verdict" — keep every file.
	if (calls.length === 0) {
		throw new Error(`prettier spec recorded no \`runFormatTest\` call: ${spec_path}`);
	}
	return calls;
};

/**
 * The path-side filter for one suite's `prettier_fixture` entry: skip a file
 * whose directory's spec expects every spec-grammar parser to reject it, and
 * the spec file itself, which is no fixture. Specs are read once per directory
 * for the lifetime of the returned function; a directory without one (Prettier
 * runs no test there) gives no verdict.
 */
export const create_prettier_fixture_skip = (
	suite: PrettierSuite
): ((path: string) => Promise<boolean>) => {
	const cache = new Map<string, Promise<PrettierFormatTestCall[] | null>>();
	const calls_for = (dir: string): Promise<PrettierFormatTestCall[] | null> => {
		let pending = cache.get(dir);
		if (!pending) {
			const spec_path = join(dir, PRETTIER_SPEC_FILENAME);
			pending = readFile(spec_path, 'utf8').then(
				(source) => evaluate_prettier_spec(source, spec_path),
				(e: unknown) => {
					if ((e as { code?: string }).code === 'ENOENT') return null;
					throw e;
				}
			);
			cache.set(dir, pending);
		}
		return pending;
	};
	return async (path) => {
		const filename = basename(path);
		// the spec is no fixture (Prettier's `getFiles` skips it): harness JS,
		// not a parse case, and one file in five of the suites' raw walk
		if (filename === PRETTIER_SPEC_FILENAME) return true;
		const dir = dirname(path);
		const calls = await calls_for(dir);
		if (calls === null) return false;
		return (
			prettier_fixture_expected_valid(filename, suite, calls, prettier_error_test_dir(dir)) ===
			false
		);
	};
};

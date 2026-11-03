/**
 * The import corpus differential — `deno task imports:corpus`: tsv's import records
 * (`tsv_ts::parse_imports`, `tsv_svelte::parse_imports`) against oxc-parser's module record
 * over real code.
 *
 * **Corpus.** Every JS/TS-family file (`.ts` / `.mts` / `.cts` / `.js` / `.mjs` / `.cjs`,
 * declaration files and `.svelte.ts` included) and every `.svelte` component of the `gates`
 * view's snapshot tiers — the `../corpora` collections, pinned by tree id. Not the view's
 * Prettier suites: formatter fixtures, many of them grammar errors oxc reports, so a verdict
 * on one says nothing about the records. Not the `robustness` view: its live working trees
 * are unpinned, and the counts here are pinned.
 *
 * **tsv's side** is one `tsv_debug imports --stdin` process (the `--profile corpus` build
 * `deno task build:check` makes, so a panic is caught and answered as one), returning the
 * JSON a binding returns: UTF-16 offsets, the specifier decoded. **oxc's side** is
 * `parseSync`'s `module` record, joined to oxc's own tree where the record says less than the
 * projection compared (below). A component's scripts are found by Svelte's `parse` (the
 * canonical parser, not tsv's), each handed to oxc as a module and its offsets moved by the
 * script's start in the component.
 *
 * **The projection** is what gro's filer reads, plus the spans: per record, in source order,
 * the kind, the statement's span, the decoded specifier and its span (quotes excluded), the
 * type-only verdict, the phase, and whether an attributes clause is present (a dynamic
 * import's options span, exactly); per file, `hasModuleSyntax`.
 *
 * **The two models differ by construction, and each difference is a named normalization**
 * (`NORMALIZATIONS`, counted per run, every one reported):
 *
 * - `quoted_specifier_span`: oxc's `ValueSpan` includes the quotes (or the backticks of a
 *   template); tsv's specifier span is the contents.
 * - `dynamic_value_from_ast`: oxc's dynamic record carries no value, only the argument's span;
 *   the value is read off oxc's `ImportExpression.source` — a string literal's value, or a
 *   substitution-free template's cooked quasi. Any other argument is computed: no specifier,
 *   as tsv has none.
 * - `duplicate_dynamic_import`: oxc's `dynamicImports` lists an `import()` inside a
 *   parenthesized assignment (`({a} = await import('./a'))`) twice, with the same span; an
 *   entry whose start and end both repeat an earlier one's is dropped.
 * - `type_only_from_ast`: oxc's record has `isType` per entry, which cannot tell
 *   `import type {A}` from `import {type A}`; the statement-level verdict is oxc's
 *   `importKind` / `exportKind`, and `specifiers` is "at least one entry, every one `isType`".
 * - `phase_from_ast`: the record has no phase; it is oxc's node's `phase`.
 * - `attributes_from_ast`: the record has no attributes; a dynamic import's span is oxc's
 *   node's `options`, and a static clause's presence is read off the source — a `with`
 *   keyword between the source literal's end and the node's end, trivia skipped — since oxc's
 *   `attributes` is empty for an empty clause (`with {}`) as for none.
 * - `indirect_export`: a local `export {a}` of an imported `a` is an indirect export entry in
 *   oxc's record — the import's module request and the import's span; dropped only when an
 *   `ImportDeclaration` of oxc's tree starts there. A `staticExports` statement with no module
 *   request is a local export, no record on either side. Every other one must be an
 *   `export … from` / `export * from` node of oxc's tree with the record's span, or the file
 *   fails.
 * - `empty_re_export`: `export {} from 'x'` has no entry, so oxc's record omits it; the
 *   record is built from oxc's `ExportNamedDeclaration`, which must have no specifiers — any
 *   other sourced export missing from the record fails the file.
 * - `import_equals_from_ast`: oxc's record omits `import x = require('y')`; tsv reports it.
 *   The record is built from oxc's top-level `TSImportEqualsDeclaration` with an external
 *   module reference — top-level only, since inside a namespace body neither side has a
 *   record (tsc rejects a static import, a re-export and an import-equals there, TS1147 /
 *   TS1194; oxc's record is top-level by construction).
 * - `top_level_await`: oxc's `hasModuleSyntax` reads import / export statements and
 *   `import.meta` only; tsv's also counts a top-level `await` / `for await` / `await using`,
 *   module-goal-only syntax. A file oxc says `false` and tsv `true` is accepted only when
 *   oxc's tree holds such an `await` outside every function, static block, field initializer,
 *   namespace body and enum.
 * - `typescript_module_forms`: oxc's `hasModuleSyntax` also counts `export =`,
 *   `export as namespace N` and `export import x = …`; tsv's counts module-goal-only syntax
 *   alone, and those three are TypeScript's CommonJS and global-script forms. A file oxc says
 *   `true` and tsv `false` is accepted only when one of the three is among oxc's top-level
 *   statements and nothing else there, nor an `import.meta`, is module syntax.
 * - `javascript_as_typescript`: tsv reads the JS family as TypeScript, so oxc is handed the
 *   matching TypeScript name (`.js` → `.ts`, `.mjs` → `.mts`, `.cjs` → `.cts`).
 *
 * The same rules tsv's collector and the fixtures' ESTree walk share (types are not walked,
 * so a type-position `import('x')` is no record on either side; ambient code is not walked)
 * need no normalization here: oxc's record leaves both out too.
 *
 * **Per file**, failing the run: a panic or a parse error on tsv's side; oxc or Svelte's
 * parser rejecting what tsv accepts; any record or `hasModuleSyntax` difference left after
 * the normalizations. An `ACCEPTED` entry excuses one file's record difference or oxc
 * rejection, naming why tsv's answer stands. **The run** also fails on an accepted entry that
 * no longer fires, on an empty corpus, and on a count off its pin (`IMPORTS_CORPUS_PINS`).
 *
 * **Probes.** Ahead of the corpus, each normalization runs over a source of its own
 * (`NORMALIZATION_PROBES`), which must reconcile the two sides AND apply that normalization,
 * so a rule the snapshot never reaches still runs every time. `type_only_from_ast`,
 * `phase_from_ast` and `attributes_from_ast` count only the applications that decide a case
 * (a type-only verdict, a phase, a present clause), so their probes prove the decisive one.
 *
 * Takes no arguments: the whole corpus runs in seconds, so every run is a full one. To look
 * at one file, `tsv_debug imports <path>`.
 *
 * @module
 */

import { extname, relative, resolve } from 'node:path';
import process from 'node:process';

import type {
	ImportRecordKind,
	ImportsSuccess
} from '../../crates/tsv_wasm/npm/facade_imports.d.ts';
import { check_node_modules } from './lib/check_node_modules.ts';
import { CORPORA_COLLECTIONS, is_collection_path } from './lib/corpora.ts';
import { CorpusLoader } from './lib/corpus.ts';
import { IMPORTS_CORPUS_PINS } from './lib/gate_counts.ts';
import { oxc_fatal_errors } from './lib/oxc.ts';
import { TsvDebugServer } from './lib/tsv_debug_server.ts';

/** A check an `ACCEPTED` entry can excuse. */
type AcceptedCheck = 'oxc_rejects' | 'records';

/**
 * The reviewed departures: one file (relative to `../corpora/collections`), the check that
 * fires on it, and why tsv's answer stands. Each must still fire, or the run fails.
 */
const ACCEPTED: ReadonlyArray<{ path: string; check: AcceptedCheck; why: string }> = [
	{
		path: 'continue/core/index.d.ts',
		check: 'oxc_rejects',
		why:
			'an initializer in an ambient context (`export const Deprecated: 1 = 1;` in a ' +
			"declaration file's namespace), which oxc rejects and tsc reports (TS1039): a " +
			'static-semantic early error tsv defers, so the file has no oxc record to compare'
	}
];

/** The named structural differences between the two models (module doc). */
const NORMALIZATIONS = [
	'quoted_specifier_span',
	'dynamic_value_from_ast',
	'duplicate_dynamic_import',
	'type_only_from_ast',
	'phase_from_ast',
	'attributes_from_ast',
	'indirect_export',
	'empty_re_export',
	'import_equals_from_ast',
	'top_level_await',
	'typescript_module_forms',
	'javascript_as_typescript'
] as const;
type Normalization = (typeof NORMALIZATIONS)[number];

/**
 * One source per normalization, graded ahead of the corpus: each must reconcile the two sides
 * and must apply its normalization, so a rule the snapshot never reaches still runs.
 */
const NORMALIZATION_PROBES: ReadonlyArray<{
	normalization: Normalization;
	path: string;
	source: string;
}> = [
	{ normalization: 'quoted_specifier_span', path: 'probe.ts', source: "import a from './a';\n" },
	{
		// both scripts, the module one second, behind a BOM and a non-ASCII character
		normalization: 'quoted_specifier_span',
		path: 'probe.svelte',
		source:
			"\uFEFF<script lang=\"ts\">\n\timport b from './b';\n\timport('./é');\n</script>\n" +
			"<p>é</p>\n<script module>\n\texport { m } from './m';\n</script>\n"
	},
	{
		normalization: 'dynamic_value_from_ast',
		path: 'probe.ts',
		source: "import(`./t`);\nimport('./s');\nimport(x);\nimport(`./${x}`);\n"
	},
	{
		normalization: 'duplicate_dynamic_import',
		path: 'probe.ts',
		source: "({ a } = await import('./a'));\n"
	},
	{
		normalization: 'type_only_from_ast',
		path: 'probe.ts',
		source:
			"import type { A } from 'a';\nimport { type B } from 'b';\nimport {} from 'c';\n" +
			"export type { D } from 'd';\nexport { type E } from 'e';\nexport type * from 'f';\n"
	},
	{
		normalization: 'phase_from_ast',
		path: 'probe.ts',
		source: "import defer * as d from 'd';\nimport source s from 's';\nimport.defer('x');\n"
	},
	{
		normalization: 'attributes_from_ast',
		path: 'probe.ts',
		source:
			"import j from './j.json' with { type: 'json' };\n" +
			"import('./k.json', { with: { type: 'json' } });\n"
	},
	{
		// an empty clause, which oxc's `attributes` cannot tell from none, behind a comment
		normalization: 'attributes_from_ast',
		path: 'probe.ts',
		source: "import {} from 'c' /* with */ with {};\nexport * from 'd' /* c */;\n"
	},
	{
		// the sourced `export * from` / `export {b} from` are compared, not dropped as indirect
		normalization: 'indirect_export',
		path: 'probe.ts',
		source:
			"import { a } from 'x';\nexport { a };\nexport * from 'y';\nexport { b } from 'z';\n" +
			"export * as n from 'w';\nconst c = 1;\nexport { c };\n"
	},
	{ normalization: 'empty_re_export', path: 'probe.ts', source: "export {} from 'e';\n" },
	{
		normalization: 'import_equals_from_ast',
		path: 'probe.ts',
		source:
			"import x = require('x');\nexport import y = require('y');\n" +
			"import type z = require('z');\nimport w = A.B;\n" +
			"namespace N { export const v = 1; import('n'); }\n"
	},
	{
		normalization: 'top_level_await',
		path: 'probe.ts',
		source: 'await x;\nasync function f() { await y; }\n'
	},
	{ normalization: 'typescript_module_forms', path: 'probe.ts', source: 'export = x;\n' },
	{
		normalization: 'typescript_module_forms',
		path: 'probe.d.ts',
		source: 'export as namespace N;\n'
	},
	{
		normalization: 'typescript_module_forms',
		path: 'probe.ts',
		source: "export import y = require('y');\nexport import v = A.B;\n"
	},
	{
		normalization: 'javascript_as_typescript',
		path: 'probe.js',
		source: "import a from './a.js';\nexport * from './b.js';\n"
	}
];

/** The nodes inside which an `await` is not a top-level one (`top_level_await`). */
const FUNCTION_LIKE = new Set([
	'FunctionDeclaration',
	'FunctionExpression',
	'ArrowFunctionExpression',
	'StaticBlock',
	'TSModuleDeclaration',
	'TSEnumDeclaration'
]);

/** How oxc's own rejection of a file is reported, the one failure `oxc_rejects` excuses. */
const OXC_REJECTS = 'oxc-parser rejects it';

const SCRIPT = /\.[cm]?[jt]s$/;
const SVELTE = /\.svelte$/;

/**
 * The projection both sides are compared on (module doc §The projection): tsv's record with
 * its attributes clause reduced to its presence, and a specifier value oxc may lack (a
 * template's cooked value).
 */
interface Projected {
	kind: ImportRecordKind;
	start: number;
	end: number;
	specifier: { value: string | null; start: number; end: number } | null;
	typeOnly: 'statement' | 'specifiers' | null;
	phase: string | null;
	attributes: boolean | { start: number; end: number } | null;
}

interface FileProjection {
	hasModuleSyntax: boolean;
	imports: Projected[];
}

/** One reply line of `tsv_debug imports --stdin`: the records as a binding returns them. */
type ImportsReply = { imports: Omit<ImportsSuccess, 'ok'> } | { error: string } | { panic: string };

// deno-lint-ignore no-explicit-any
type Node = any;

if (!('Deno' in globalThis)) {
	console.error('Error: imports_corpus.ts runs under Deno (`deno task imports:corpus`).');
	process.exit(1);
}

await check_node_modules();
const server = await TsvDebugServer.start<{ path: string; source: string }, ImportsReply>(
	'imports'
);
// dynamic, so a missing node_modules reports through the preflight above
const { parseSync } = await import('oxc-parser');
const { parse: svelte_parse } = await import('svelte/compiler');
const { createRequire } = await import('node:module');
const require = createRequire(import.meta.url);
const versions = {
	oxc_parser: (require('oxc-parser/package.json') as { version: string }).version,
	svelte: (require('svelte/package.json') as { version: string }).version
};

const started = performance.now();
const collections_root = resolve(CORPORA_COLLECTIONS);
const files = (await new CorpusLoader('gates', { only: is_collection_path }).load(() => {})).filter(
	(f) => SCRIPT.test(f.path) || SVELTE.test(f.path)
);

console.log(
	`imports corpus: tsv parse_imports vs oxc-parser ${versions.oxc_parser} module record ` +
		`(svelte ${versions.svelte} finds the scripts); ${files.length} file(s) from ` +
		CORPORA_COLLECTIONS
);

const failures: Array<string> = [];
const accepted_fired = new Set<string>();
const applied = new Map<Normalization, number>(NORMALIZATIONS.map((n) => [n, 0]));
const note = (n: Normalization, count = 1): void => {
	applied.set(n, applied.get(n)! + count);
};
const kinds = new Map<string, number>();
let records = 0;
let module_syntax = 0;
let svelte_files = 0;

/**
 * One file's verdict: `null` when the two sides agree, else the check that fired and why.
 * When `tally` is set, a file both sides parse is counted into the run's tallies — its
 * records, their kinds, its module syntax, and whether it is a component — so every tally
 * counts the same files.
 */
async function grade(
	file_path: string,
	source: string,
	tally: boolean
): Promise<{ check: AcceptedCheck | null; message: string } | null> {
	let reply: ImportsReply;
	try {
		reply = await server.ask({ path: file_path, source });
	} catch (e) {
		console.error(`✗ ${e instanceof Error ? e.message : e}, on ${file_path}`);
		process.exit(1);
	}
	if ('panic' in reply) return { check: null, message: `tsv panicked: ${reply.panic}` };
	if ('error' in reply) {
		return { check: null, message: `tsv's parser rejects it: ${reply.error.split('\n')[0]}` };
	}
	const is_svelte = SVELTE.test(file_path);
	const theirs = is_svelte ? oxc_svelte(source) : oxc_script(file_path, source, 0, null);
	if (typeof theirs === 'string') {
		// only oxc's own rejection can be accepted; a tree the harness cannot join is its bug
		return { check: theirs.startsWith(OXC_REJECTS) ? 'oxc_rejects' : null, message: theirs };
	}
	const ours = reply.imports;
	if (tally) {
		if (is_svelte) svelte_files++;
		records += ours.imports.length;
		if (ours.hasModuleSyntax) module_syntax++;
		for (const r of ours.imports) kinds.set(r.kind, (kinds.get(r.kind) ?? 0) + 1);
	}
	const ours_projected = ours.imports.map((r): Projected => ({
		...r,
		attributes: r.kind === 'dynamic' ? r.attributes : r.attributes !== null
	}));
	const difference = first_difference(ours_projected, theirs.imports);
	if (difference !== null) return { check: 'records', message: difference };
	if (ours.hasModuleSyntax !== theirs.hasModuleSyntax) {
		return {
			check: 'records',
			message: `hasModuleSyntax: tsv ${ours.hasModuleSyntax}, oxc ${theirs.hasModuleSyntax}`
		};
	}
	return null;
}

// every normalization first, on a source that needs it: a rule the corpus never exercises
// is still run, and must still reconcile the two sides
for (const probe of NORMALIZATION_PROBES) {
	const before = applied.get(probe.normalization)!;
	const verdict = await grade(probe.path, probe.source, false);
	if (verdict !== null) {
		failures.push(`normalization probe ${probe.normalization}: ${verdict.message}`);
	} else if (applied.get(probe.normalization) === before) {
		failures.push(`normalization probe ${probe.normalization}: the normalization never applied`);
	}
}
const probe_counts = new Map(applied);

for (const file of files) {
	const path = relative(collections_root, file.path);
	// a lone surrogate has no UTF-8 spelling, so both sides read the replaced string
	const verdict = await grade(file.path, file.content.toWellFormed(), true);
	if (verdict === null) continue;
	const { check, message } = verdict;
	if (check !== null && ACCEPTED.some((a) => a.check === check && a.path === path)) {
		accepted_fired.add(`${check}\t${path}`);
	} else {
		failures.push(`${path}: ${message}`);
	}
}

try {
	await server.close();
} catch (e) {
	failures.push(e instanceof Error ? e.message : String(e));
}

if (files.length === 0) failures.push('the corpus is empty — nothing was graded');
for (const a of ACCEPTED) {
	if (!accepted_fired.has(`${a.check}\t${a.path}`)) {
		failures.push(`stale ACCEPTED entry (${a.check}) ${a.path}: it no longer fires`);
	}
}
const counts = { files: files.length, records };
for (const key of Object.keys(IMPORTS_CORPUS_PINS) as Array<keyof typeof IMPORTS_CORPUS_PINS>) {
	if (counts[key] !== IMPORTS_CORPUS_PINS[key]) {
		failures.push(
			`pinned count mismatch: ${key} is ${counts[key]}, pinned ${IMPORTS_CORPUS_PINS[key]} ` +
				'(benches/js/lib/gate_counts.ts IMPORTS_CORPUS_PINS; docs/gate_counts.md §Update ritual)'
		);
	}
}

const by_kind = [...kinds].map(([kind, n]) => `${kind} ${n}`).join(', ');
const by_normalization = [...applied]
	.map(([n, count]) => `${n} ${count - probe_counts.get(n)!}`)
	.join(', ');
const seconds = ((performance.now() - started) / 1000).toFixed(1);
console.log(
	`${files.length} file(s) (${svelte_files} Svelte): ${records} record(s)` +
		(by_kind ? ` (${by_kind})` : '') +
		`, ${module_syntax} with module syntax, ${accepted_fired.size} accepted departure(s) — ` +
		`${seconds}s\nnormalizations applied over the corpus: ${by_normalization}`
);
if (failures.length > 0) {
	console.error(`\n✗ ${failures.length} failure(s):`);
	for (const f of failures) console.error(`  ${f}`);
	process.exit(1);
}
console.log("✓ tsv's import records agree with oxc-parser's module record, normalized");

/** The first record where the two projections part, or `null` when they agree. */
function first_difference(ours: Projected[], theirs: Projected[]): string | null {
	for (let i = 0; i < Math.max(ours.length, theirs.length); i++) {
		const a = JSON.stringify(ours[i] ?? null);
		const b = JSON.stringify(theirs[i] ?? null);
		if (a !== b) return `record ${i}: tsv ${a}, oxc ${b}`;
	}
	return null;
}

/** oxc's projection of a component's scripts, found by Svelte's `parse`, or why there is none. */
function oxc_svelte(source: string): FileProjection | string {
	// Svelte's `parse` strips a leading BOM, and its offsets index the stripped string, as
	// tsv's Svelte records do
	const text = source.startsWith('\uFEFF') ? source.slice(1) : source;
	let ast: Node;
	try {
		ast = svelte_parse(text, { modern: true });
	} catch (e) {
		return `svelte/compiler rejects it: ${e instanceof Error ? e.message.split('\n')[0] : e}`;
	}
	const merged: FileProjection = { hasModuleSyntax: false, imports: [] };
	for (const script of [ast.module, ast.instance]) {
		if (!script) continue;
		const { start, end } = script.content as { start: number; end: number };
		const projected = oxc_script('script.ts', text.slice(start, end), start, 'module');
		if (typeof projected === 'string') return projected;
		merged.hasModuleSyntax ||= projected.hasModuleSyntax;
		merged.imports.push(...projected.imports);
	}
	merged.imports.sort((a, b) => a.start - b.start);
	return merged;
}

/**
 * oxc's projection of one script whose offsets start at `base` in the document, or why there
 * is none. `goal` null reads the file as tsv's unset goal does: module, retried as a script.
 */
function oxc_script(
	path: string,
	text: string,
	base: number,
	goal: 'module' | null
): FileProjection | string {
	const name = typescript_name(path);
	const by_extension = /\.m[jt]s$/.test(path) ? 'module' : goal;
	let result = parseSync(name, text, { sourceType: 'module' });
	let fatal = oxc_fatal_errors(result.errors);
	if (fatal.length > 0 && by_extension === null) {
		const script = parseSync(name, text, { sourceType: 'script' });
		if (oxc_fatal_errors(script.errors).length === 0) {
			result = script;
			fatal = [];
		}
	}
	if (fatal.length > 0) return `${OXC_REJECTS}: ${fatal[0]!.message}`;
	const program = result.program as Node;
	const module = result.module;
	const nodes = index_nodes(program);
	const at = (type: string, start: number): Node | undefined => nodes.get(`${type}@${start}`);
	const shift = (n: number): number => n + base;
	const unquote = (span: { start: number; end: number }) => {
		note('quoted_specifier_span');
		return { start: shift(span.start + 1), end: shift(span.end - 1) };
	};
	// each counts only the applications that decide a case, so a probe proves the decisive one
	const type_only = (verdict: Projected['typeOnly']): Projected['typeOnly'] => {
		if (verdict !== null) note('type_only_from_ast');
		return verdict;
	};
	const phase = (value: string | null): string | null => {
		if (value !== null) note('phase_from_ast');
		return value;
	};
	const clause = (node: Node): boolean => {
		const present = has_with_clause(text, node.source.end, node.end);
		if (present) note('attributes_from_ast');
		return present;
	};
	const imports: Projected[] = [];

	for (const s of module.staticImports) {
		const node = at('ImportDeclaration', s.start);
		if (!node) return `no ImportDeclaration in oxc's tree at ${s.start}`;
		imports.push({
			kind: 'static',
			start: shift(s.start),
			end: shift(s.end),
			specifier: { value: s.moduleRequest.value, ...unquote(s.moduleRequest) },
			typeOnly: type_only(
				node.importKind === 'type'
					? 'statement'
					: s.entries.length > 0 && s.entries.every((e) => e.isType)
						? 'specifiers'
						: null
			),
			phase: phase(node.phase ?? null),
			attributes: clause(node)
		});
	}

	const re_export_starts = new Set<number>();
	for (const s of module.staticExports) {
		const node = at('ExportNamedDeclaration', s.start) ?? at('ExportAllDeclaration', s.start);
		if (!node?.source) {
			// a local export is no request; an entry with one and no `export … from` node at
			// its start is an indirect export only when it carries the import's span
			if (s.entries.every((e) => e.moduleRequest === null)) continue;
			if (!at('ImportDeclaration', s.start)) {
				return `an export entry at ${s.start} with a module request joins no node of oxc's tree`;
			}
			note('indirect_export');
			continue;
		}
		if (node.end !== s.end) {
			return `a re-export at ${s.start} ends at ${s.end} in oxc's record, ${node.end} in its tree`;
		}
		const request = s.entries[0]?.moduleRequest;
		if (!request) return `a re-export at ${s.start} with no module request in oxc's record`;
		re_export_starts.add(s.start);
		imports.push({
			kind: 're_export',
			start: shift(s.start),
			end: shift(s.end),
			specifier: { value: request.value, ...unquote(request) },
			typeOnly: type_only(
				node.exportKind === 'type'
					? 'statement'
					: s.entries.length > 0 && s.entries.every((e) => e.isType)
						? 'specifiers'
						: null
			),
			phase: null,
			attributes: clause(node)
		});
	}

	// what the record leaves out, read off oxc's top-level statements
	for (const statement of program.body as Node[]) {
		if (
			(statement.type === 'ExportNamedDeclaration' || statement.type === 'ExportAllDeclaration') &&
			statement.source &&
			!re_export_starts.has(statement.start)
		) {
			if (statement.type !== 'ExportNamedDeclaration' || statement.specifiers.length !== 0) {
				return `a re-export at ${statement.start} with specifiers, missing from oxc's record`;
			}
			note('empty_re_export');
			imports.push({
				kind: 're_export',
				start: shift(statement.start),
				end: shift(statement.end),
				specifier: { value: statement.source.value, ...unquote(statement.source) },
				typeOnly: type_only(statement.exportKind === 'type' ? 'statement' : null),
				phase: null,
				attributes: clause(statement)
			});
		}
		const equals =
			statement.type === 'TSImportEqualsDeclaration'
				? statement
				: statement.type === 'ExportNamedDeclaration' &&
					  statement.declaration?.type === 'TSImportEqualsDeclaration'
					? statement.declaration
					: null;
		if (equals && equals.moduleReference?.type === 'TSExternalModuleReference') {
			note('import_equals_from_ast');
			const literal = equals.moduleReference.expression;
			imports.push({
				kind: 'import_equals',
				start: shift(statement.start),
				end: shift(statement.end),
				specifier: { value: literal.value, ...unquote(literal) },
				typeOnly: equals.importKind === 'type' ? 'statement' : null,
				phase: null,
				attributes: false
			});
		}
	}

	const dynamic_spans = new Set<string>();
	for (const d of module.dynamicImports) {
		const span = `${d.start}-${d.end}`;
		if (dynamic_spans.has(span)) {
			note('duplicate_dynamic_import');
			continue;
		}
		dynamic_spans.add(span);
		const node = at('ImportExpression', d.start);
		if (!node) return `no ImportExpression in oxc's tree at ${d.start}`;
		note('dynamic_value_from_ast');
		if (node.options) note('attributes_from_ast');
		const argument = node.source;
		let specifier: Projected['specifier'] = null;
		if (argument.type === 'Literal' && typeof argument.value === 'string') {
			specifier = { value: argument.value, ...unquote(d.moduleRequest) };
		} else if (argument.type === 'TemplateLiteral' && argument.expressions.length === 0) {
			specifier = { value: argument.quasis[0].value.cooked ?? null, ...unquote(d.moduleRequest) };
		}
		imports.push({
			kind: 'dynamic',
			start: shift(d.start),
			end: shift(d.end),
			specifier,
			typeOnly: null,
			phase: phase(node.phase ?? null),
			attributes: node.options
				? { start: shift(node.options.start), end: shift(node.options.end) }
				: null
		});
	}

	imports.sort((a, b) => a.start - b.start);
	let has_module_syntax = module.hasModuleSyntax;
	if (has_module_syntax && module.importMetas.length === 0) {
		const statements = program.body as Node[];
		const is_typescript_form = (statement: Node): boolean =>
			statement.type === 'TSExportAssignment' ||
			statement.type === 'TSNamespaceExportDeclaration' ||
			(statement.type === 'ExportNamedDeclaration' &&
				statement.declaration?.type === 'TSImportEqualsDeclaration');
		const is_module_statement = (statement: Node): boolean =>
			!is_typescript_form(statement) &&
			/^(Import|Export(Named|Default|All))Declaration$/.test(statement.type);
		if (statements.some(is_typescript_form) && !statements.some(is_module_statement)) {
			note('typescript_module_forms');
			has_module_syntax = false;
		}
	}
	if (!has_module_syntax && has_top_level_await(program)) {
		note('top_level_await');
		has_module_syntax = true;
	}
	return { hasModuleSyntax: has_module_syntax, imports };
}

/**
 * Whether a `with` keyword opens an attributes clause between a static import's or a
 * re-export's source literal (ending at `from`) and the statement's end (`to`), trivia
 * skipped. Read off the source, since oxc's `attributes` is empty for an empty clause
 * (`with {}`), as for none.
 */
function has_with_clause(text: string, from: number, to: number): boolean {
	let i = from;
	while (i < to) {
		if (/\s/.test(text[i]!)) {
			i++;
		} else if (text.startsWith('//', i)) {
			const newline = text.slice(i, to).search(/[\n\r\u2028\u2029]/);
			i = newline === -1 ? to : i + newline;
		} else if (text.startsWith('/*', i)) {
			const close = text.indexOf('*/', i + 2);
			i = close === -1 || close >= to ? to : close + 2;
		} else {
			return text.startsWith('with', i) && !/[\w$]/.test(text[i + 4] ?? '');
		}
	}
	return false;
}

/** The TypeScript name oxc is handed for `path` (`javascript_as_typescript`). */
function typescript_name(path: string): string {
	const extension = extname(path);
	const typescript = { '.js': '.ts', '.mjs': '.mts', '.cjs': '.cts' }[extension];
	if (typescript === undefined) return path;
	note('javascript_as_typescript');
	return path.slice(0, -extension.length) + typescript;
}

/** Every node of oxc's tree by `type@start`. */
function index_nodes(root: Node): Map<string, Node> {
	const nodes = new Map<string, Node>();
	const visit = (value: Node): void => {
		if (Array.isArray(value)) {
			for (const item of value) visit(item);
		} else if (value !== null && typeof value === 'object') {
			if (typeof value.type === 'string' && typeof value.start === 'number') {
				const key = `${value.type}@${value.start}`;
				if (!nodes.has(key)) nodes.set(key, value);
			}
			for (const key in value) {
				if (key !== 'parent') visit(value[key]);
			}
		}
	};
	visit(root);
	return nodes;
}

/** Whether oxc's tree holds a top-level `await`, `for await` or `await using`. */
function has_top_level_await(root: Node): boolean {
	const visit = (value: Node): boolean => {
		if (Array.isArray(value)) return value.some(visit);
		if (value === null || typeof value !== 'object') return false;
		const type = value.type;
		if (FUNCTION_LIKE.has(type)) return false;
		if (type === 'AwaitExpression') return true;
		if (type === 'ForOfStatement' && value.await === true) return true;
		if (type === 'VariableDeclaration' && value.kind === 'await using') return true;
		const field = type === 'PropertyDefinition' || type === 'AccessorProperty';
		for (const key in value) {
			if (key === 'parent' || (field && key === 'value')) continue;
			if (visit(value[key])) return true;
		}
		return false;
	};
	return visit(root);
}

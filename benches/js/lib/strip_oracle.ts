/**
 * The type strip's oracle side, shared by the strip fixtures (`strip_fixtures.ts`) and the
 * strip corpus differential (`strip_corpus.ts`): ts-blank-space's output and refusals in
 * tsv's terms, and the JS-validity guard every claimed or graded output passes.
 *
 * - **The derivation** (`derive`): tsv's expected output is ts-blank-space's, run over the
 *   input with every CR, U+2028 and U+2029 folded to LF, each one then restored at its
 *   UTF-16 offset. ts-blank-space tests "spans lines" against LF alone, so the fold is what
 *   makes it see every ECMAScript line terminator at the hazard sites, and the restore is
 *   tsv's contract that every terminator survives verbatim. On an input holding none the
 *   derivation IS ts-blank-space's output, byte for byte.
 * - **Refusals.** When ts-blank-space reports a node through its `onError` callback the
 *   outcome is a refusal, recorded as `refusal.txt` spells it: tsv's `UnsupportedKind` name
 *   and the node's start and end as `line:column` (1-based line, 0-based UTF-16 column,
 *   ECMAScript line terminators — tsv's `WirePoint`), the first reported node in source
 *   order.
 * - **JS validity** (`js_parse_failure`): oxc-parser in JS mode, semantic errors included.
 *   tsv's own parser accepts TypeScript, so this is the check that an output is JS at all.
 *
 * Imports npm packages, so the scripts load it dynamically, after their `node_modules`
 * preflight. Runs under Deno and Node alike.
 *
 * @module
 */

import { createRequire } from 'node:module';

import { oxc_fatal_errors } from './oxc.ts';

const { default: ts } = await import('typescript');
const { blankSourceFile } = await import('ts-blank-space');
const { parseSync: oxc_parse } = await import('oxc-parser');

const require = createRequire(import.meta.url);

/** The pinned oracle versions, for a run's header. */
export const ORACLE_VERSIONS = {
	ts_blank_space: (require('ts-blank-space/package.json') as { version: string }).version,
	typescript: ts.version,
	oxc_parser: (require('oxc-parser/package.json') as { version: string }).version
};

/** tsv's `UnsupportedKind` variant names: what a refusal record may name. */
export const UNSUPPORTED_KINDS = [
	'Enum',
	'ValueNamespace',
	'ParameterProperty',
	'ImportEquals',
	'ExportEquals',
	'TypeAssertion',
	'AssertionGrouping'
] as const;
export type UnsupportedKind = (typeof UNSUPPORTED_KINDS)[number];

/** What one strip produced: JS, or a refusal record (`Kind\tline:column\tline:column\n`). */
export type Outcome = { code: string } | { refusal: string };

export const is_terminator = (c: string): boolean =>
	c === '\n' || c === '\r' || c === '\u2028' || c === '\u2029';

/** The `line:column` of UTF-16 offset `offset` — tsv's `WirePoint` (CRLF is one terminator). */
export function point(text: string, offset: number): string {
	let line = 1;
	let column = 0;
	for (let i = 0; i < offset; i++) {
		const c = text[i]!;
		const breaks =
			c === '\n' || c === '\u2028' || c === '\u2029' || (c === '\r' && text[i + 1] !== '\n');
		if (breaks) {
			line++;
			column = 0;
		} else {
			column++;
		}
	}
	return `${line}:${column}`;
}

/**
 * ts-blank-space on `text`, with its `onError` nodes collected into a refusal record whose
 * points are read in `positions_in` (the unfolded input). Throws when TypeScript reports a
 * parse diagnostic: ts-blank-space's output then says nothing about valid input.
 */
export function run_ts_blank_space(text: string, positions_in: string): Outcome {
	const source_file = ts.createSourceFile(
		'input.ts',
		text,
		{
			languageVersion: ts.ScriptTarget.ESNext,
			impliedNodeFormat: ts.ModuleKind.ESNext,
			jsDocParsingMode: ts.JSDocParsingMode.ParseNone
		},
		false,
		ts.ScriptKind.TS
	);
	// `parseDiagnostics` is internal: the only parse verdict a SourceFile carries
	const diagnostics = (source_file as unknown as { parseDiagnostics: Array<unknown> })
		.parseDiagnostics;
	if (diagnostics.length > 0) {
		throw new Error(`TypeScript reports ${diagnostics.length} parse diagnostic(s)`);
	}
	const refused: Array<{ kind: string; start: number; end: number }> = [];
	const code = blankSourceFile(source_file, (node) => {
		refused.push({
			kind: unsupported_kind(node),
			start: node.getStart(source_file),
			end: node.end
		});
	});
	if (refused.length === 0) return { code };
	refused.sort((a, b) => a.start - b.start);
	const first = refused[0]!;
	return {
		refusal: `${first.kind}\t${point(positions_in, first.start)}\t${point(positions_in, first.end)}\n`
	};
}

/** tsv's `UnsupportedKind` name for a node ts-blank-space refuses. */
function unsupported_kind(node: import('typescript').Node): UnsupportedKind {
	const k = ts.SyntaxKind;
	switch (node.kind) {
		case k.EnumDeclaration:
			return 'Enum';
		case k.ModuleDeclaration:
			return 'ValueNamespace';
		case k.PublicKeyword:
		case k.PrivateKeyword:
		case k.ProtectedKeyword:
		case k.ReadonlyKeyword:
		case k.OverrideKeyword:
			return 'ParameterProperty';
		case k.ImportEqualsDeclaration:
			return 'ImportEquals';
		case k.ExportAssignment:
			return 'ExportEquals';
		case k.TypeAssertionExpression:
			return 'TypeAssertion';
		case k.AsExpression:
		case k.SatisfiesExpression:
			return 'AssertionGrouping';
		default:
			throw new Error(`ts-blank-space refused an unmapped node: ${ts.SyntaxKind[node.kind]}`);
	}
}

/**
 * The derivation: ts-blank-space over the terminator-folded input, terminators restored. With
 * nothing folded its output is returned as is, even one that changed the UTF-16 length: that is
 * a wrong output, which the contract guard refuses to claim and an override stands against.
 */
export function derive(input: string): Outcome {
	const folded = input.replace(/[\r\u2028\u2029]/g, '\n');
	const outcome = run_ts_blank_space(folded, input);
	if ('refusal' in outcome || folded === input) return outcome;
	const code = outcome.code;
	if (code.length !== input.length) {
		throw new Error('ts-blank-space changed the UTF-16 length');
	}
	let restored = '';
	for (let i = 0; i < input.length; i++) {
		const c = input[i]!;
		if (c === '\r' || c === '\u2028' || c === '\u2029') {
			if (code[i] !== '\n') throw new Error(`ts-blank-space moved the terminator at ${i}`);
			restored += c;
		} else {
			restored += code[i];
		}
	}
	return { code: restored };
}

/**
 * Why `code` is not a JS module, or `null` when it is: oxc-parser in JS mode, semantic errors
 * included. `filename` only labels the parse.
 */
export function js_parse_failure(code: string, filename: string): string | null {
	// read `errors` once into a local (lib/oxc.ts: a getter on the WASI binding)
	const errors = oxc_fatal_errors(
		oxc_parse(filename, code, { lang: 'js', sourceType: 'module', showSemanticErrors: true }).errors
	);
	if (errors.length === 0) return null;
	return errors.map((e) => e.message ?? 'unknown error').join('; ');
}

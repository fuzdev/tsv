/**
 * oxc-parser's RAW TRANSFER mode (`experimentalRawTransfer`) — the `oxc-parser-raw` row.
 *
 * The same native addon and the same parse as `oxc-parser`'s default row (`lib/oxc.ts`),
 * with a different way across the boundary: instead of serializing the AST to a JSON
 * string in Rust and `JSON.parse`-ing it in JS, oxc writes it into a shared buffer and
 * a generated JS deserializer (`src-js/generated/deserialize/ts.js`) builds the JS
 * objects straight from those bytes. It is EAGER: `parseSync` deserializes the whole
 * program (with its comments and module record) before it returns, and the result's
 * getters hand back what was already built — the buffer is reused by the next parse,
 * so nothing can be left lazy. So the row materializes the full AST exactly as the
 * default row does, and the trees are identical: verified byte-for-byte against the
 * default path, `BigInt` / `RegExp` literal values included, and graded on every file
 * by the pre-flight's byte parity (`sibling_outputs_must_match`).
 *
 * Not to be confused with `experimentalLazy`, which defers deserialization behind a
 * visitor and is setup-dominated (docs/benchmarks.md §Fairness caveats) — that mode
 * has no row.
 *
 * Native only, and not on every runtime: `rawTransferSupported()` is false on 32-bit
 * and big-endian hosts, Node < 22, Deno < 2, the WASI binding, and Bun (oxc's own
 * runtime list). Init fails there, so the row is ABSENT with the reason in the report's
 * `unavailable` — its own impl slot rather than a gate inside `lib/oxc.ts`, since the
 * registry's unit of absence is the impl and a gated-off row would otherwise vanish
 * from one runtime's report with nothing to say why.
 *
 * The option is undocumented upstream — absent from `ParserOptions` in `index.d.ts`
 * and from the README, present in `src-js/index.js` — so the module is typed here.
 *
 * Supports: Parse only (TypeScript, JS).
 */

import { BaseImplementation, type Language, LANGUAGE_EXTENSIONS, type ParseGoal } from './types.ts';
import type { OxcVersions } from './versions.ts';
import { assert_oxc_rejects_invalid, type OxcDiagnostic, oxc_fatal_errors } from './oxc.ts';
import { assert_parse_goal_swings, assert_parser_rejects_invalid } from './reject_probe.ts';

interface OxcRawParseOptions {
	sourceType: ParseGoal;
	experimentalRawTransfer: true;
}

/** The oxc-parser surface this row reads, `experimentalRawTransfer` included. */
interface OxcRawParserModule {
	parseSync: (
		filename: string,
		source: string,
		options?: { sourceType?: ParseGoal; experimentalRawTransfer?: boolean }
	) => { program: unknown; errors: OxcDiagnostic[] };
	rawTransferSupported: () => boolean;
}

/** One options bag per goal, built once; every call passes a goal (`OXC_GOAL_OPTIONS`'s rule). */
const OXC_RAW_OPTIONS: Readonly<Record<ParseGoal, OxcRawParseOptions>> = {
	module: { sourceType: 'module', experimentalRawTransfer: true },
	script: { sourceType: 'script', experimentalRawTransfer: true }
};

/**
 * Covers every node kind the payload probe needs to cross the boundary in both modes:
 * a `BigInt` and a `RegExp` literal (the values the default path patches in after
 * `JSON.parse`), a comment, an import, a decorator, a type, and a template.
 */
const PAYLOAD_PROBE_SOURCE =
	"import { a } from 'a';\n" +
	'// comment\n' +
	'@dec class C<T> { m(x: T): T { return x; } }\n' +
	'const n = 10n, r = /ab+c/gi, t = `x${a}y`;\n';

const payload_text = (program: unknown): string =>
	JSON.stringify(program, (_key, value: unknown) =>
		typeof value === 'bigint'
			? `${value}n`
			: value instanceof RegExp
				? `/${value.source}/${value.flags}`
				: value
	);

export class OxcRawImplementation extends BaseImplementation {
	readonly versions: OxcVersions;
	private _parser: OxcRawParserModule | null = null;

	readonly parse_languages: ReadonlyArray<Language> = ['typescript'];
	readonly format_languages: ReadonlyArray<Language> = [];

	constructor(versions: OxcVersions) {
		super();
		this.versions = versions;
	}

	async init(): Promise<void> {
		const parser = (await import('oxc-parser')) as unknown as OxcRawParserModule;
		if (!parser.rawTransferSupported()) {
			throw new Error(
				'oxc-parser raw transfer is not supported on this runtime (`rawTransferSupported()` is false)'
			);
		}
		this._parser = parser;

		// The raw module's own vocabulary, then this wrapper's call at every goal, then the
		// goal axis itself — the probes the default row runs (`lib/oxc.ts`).
		assert_oxc_rejects_invalid(parser, 'oxc-parser-raw');
		assert_parser_rejects_invalid('oxc-parser-raw', this.parse_languages, (s, l, goal) =>
			this.parse(s, l, goal)
		);
		assert_parse_goal_swings('oxc-parser-raw', (s, goal) => this.parse(s, 'typescript', goal));

		// And that the option LANDED. It is undocumented and an unknown key is ignored
		// silently, so a rename would leave this row timing the JSON path under the raw
		// name. Both paths return a plain object of getters, so the tell is what the
		// `program` getter DOES: the default path's runs `jsonParseAst` on first access
		// (`src-js/wrap.js`), raw transfer's returns a tree built before `parseSync`
		// returned. The default path is read too, so a renamed `jsonParseAst` fails this
		// as "no longer discriminates" instead of passing it vacuously.
		const raw = parser.parseSync('file.ts', PAYLOAD_PROBE_SOURCE, OXC_RAW_OPTIONS.module);
		const json = parser.parseSync('file.ts', PAYLOAD_PROBE_SOURCE, { sourceType: 'module' });
		const getter_source = (result: object): string =>
			String(Object.getOwnPropertyDescriptor(result, 'program')?.get ?? '');
		if (!getter_source(json).includes('jsonParseAst')) {
			throw new Error(
				"oxc-parser-raw: the default path's `program` getter no longer calls `jsonParseAst`, " +
					'so the check that `experimentalRawTransfer` landed no longer discriminates — re-read ' +
					"oxc-parser's `src-js/wrap.js` and `src-js/raw-transfer/eager.js`"
			);
		}
		if (getter_source(raw).includes('jsonParseAst')) {
			throw new Error(
				'oxc-parser-raw: `experimentalRawTransfer` did not land — the result is the default ' +
					"path's `wrap()` object, so this row would time JSON transfer under the raw name"
			);
		}
		// The payload half: a raw tree that differs from the JSON one would be a different
		// product under the same tier (`PARSE_PAYLOAD_TIERS`). The pre-flight's per-file byte
		// parity grades the same over the whole corpus; this fails at init, on a probe that
		// carries the values the default path patches in after `JSON.parse`.
		if (payload_text(raw.program) !== payload_text(json.program)) {
			throw new Error(
				'oxc-parser-raw: the raw-transfer AST differs from the default (JSON) path on the ' +
					'payload probe, so the row no longer times the same product as `oxc-parser`'
			);
		}
	}

	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		if (!this._parser) throw new Error('OXC raw parser not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`OXC raw parser does not support ${language}`);
		}
		const result = this._parser.parseSync(
			`file${LANGUAGE_EXTENSIONS[language]}`,
			source,
			OXC_RAW_OPTIONS[goal ?? 'module']
		);
		// Read once, fatal entries only — `oxc_fatal_errors`, shared with both oxc rows.
		const errors = oxc_fatal_errors(result.errors);
		if (errors.length > 0) {
			throw new Error(`Parse errors: ${JSON.stringify(errors)}`);
		}
		// Already deserialized in full by `parseSync` (see the module doc); the getter
		// returns the built tree.
		return result.program;
	}

	format(_source: string, _language: Language): string {
		throw new Error('oxc-parser-raw is parse-only');
	}

	dispose(): void {
		this._parser = null;
	}
}

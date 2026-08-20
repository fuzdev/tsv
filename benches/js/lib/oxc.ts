/**
 * OXC implementation wrappers (oxc-parser + oxfmt)
 *
 * oxc-parser: Fast TypeScript/JS parser
 * oxfmt: Fast TypeScript/JS/CSS/Svelte formatter (Svelte is experimental as of 0.49)
 */

import {
	BaseImplementation,
	type Language,
	LANGUAGE_EXTENSIONS,
	LANGUAGES,
	type ParseGoal
} from './types.ts';
import type { OxcVersions } from './versions.ts';
import { assert_format_config_landed, FORMAT_CONFIG_PROBES } from './format_config_probe.ts';

/**
 * One entry of an oxc `errors` array — the two fields this wrapper reads.
 *
 * oxc's diagnostics carry a SEVERITY (`Error` | `Warning` | `Advice`), which is
 * what `oxc_fatal_errors` below exists to read.
 */
export interface OxcDiagnostic {
	severity?: string;
	message?: string;
}

/** oxc-parser module types */
interface OxcParserModule {
	parseSync: (
		filename: string,
		source: string,
		options?: { sourceType?: 'script' | 'module' }
	) => { program: unknown; errors: OxcDiagnostic[] };
}

/** oxfmt module types (the subset of oxfmt's real option surface the bench sets) */
interface OxfmtFormatOptions {
	useTabs?: boolean;
	printWidth?: number;
	singleQuote?: boolean;
	trailingComma?: 'all' | 'es5' | 'none';
	/** Enable experimental Svelte support — `{}` accepts defaults. */
	svelte?: boolean | Record<string, unknown>;
}

interface OxfmtModule {
	format: (
		filename: string,
		source: string,
		options?: OxfmtFormatOptions
	) => Promise<{ code: string; errors: OxcDiagnostic[] }>;
}

/**
 * oxc severities that are NOT a rejection.
 *
 * Stated as the non-fatal set rather than as the fatal one, and the polarity is the
 * whole point: an allowlist of `'Error'` would turn an upstream RENAME of that
 * value into a row that accepts every file and publishes a fabricated 100%
 * coverage — the same shape as the WASI binding's consume-once `errors` getter
 * (benches/js/CLAUDE.md §Known Issues). Under this spelling an unrecognized
 * severity counts as a rejection, which is what the wrapper did before it filtered
 * at all, so the dangerous direction is unreachable by construction and the init
 * probe below only has to catch the conservative one.
 */
const OXC_NON_FATAL_SEVERITIES: ReadonlySet<string> = new Set(['Warning', 'Advice']);

/**
 * The entries of an oxc `errors` array that mean the tool REJECTED its input.
 *
 * Counting the array's LENGTH instead scores a merely warned-about file as a
 * rejection — under-reporting oxc's coverage and, in the bench's default
 * intersection mode, dropping that file out of the set every row in the group is
 * timed on. `lib/yuku.ts` filters its diagnostics for exactly this reason
 * ("treating a warning/hint as a failure would under-report coverage"); this is
 * that rule for oxc, in ONE place for both bindings and both operations, on the
 * same argument that gives yuku's two bindings one wrapper class.
 *
 * Measured when it was written: across 52,106 conformance-corpus files (test262,
 * the tsc corpus, prettier's ts+js suites) every diagnostic oxc produced was
 * `Error`, so this moves no published number today. It makes the accept definition
 * correct rather than accidentally correct.
 *
 * ⚠️ Read the array ONCE into a local before calling — on the WASI binding
 * `result.errors` is a consume-once getter (`lib/oxc_wasm.ts`).
 */
export function oxc_fatal_errors(
	errors: ReadonlyArray<OxcDiagnostic> | undefined
): OxcDiagnostic[] {
	if (!errors) return [];
	return errors.filter(
		(e) => e.severity === undefined || !OXC_NON_FATAL_SEVERITIES.has(e.severity)
	);
}

/**
 * Assert `parser` still reports a genuine syntax error as FATAL, failing the impl
 * loudly when it doesn't.
 *
 * The counterpart to the polarity argument above: that spelling makes a *fabricated
 * accept* unreachable through a renamed severity, but it cannot see oxc moving
 * parse errors themselves onto a non-fatal severity — after which every file would
 * read as parsed. oxc reports nothing about its own diagnostic vocabulary, so the
 * check is behavioral, like `lib/swc.ts`'s decorator probe and `lib/yuku.ts`'s
 * option probes.
 *
 * Takes the MODULE rather than a detached `parseSync`, so the call keeps its
 * receiver on either binding.
 *
 * @param parser - the binding to probe
 * @param binding - the row-facing name, so the throw names which one failed
 */
export function assert_oxc_rejects_invalid(
	parser: Pick<OxcParserModule, 'parseSync'>,
	binding: string
): void {
	// Invalid at every goal, and shallow enough that no grammar change makes it valid.
	const result = parser.parseSync('file.ts', 'const x = ;');
	if (oxc_fatal_errors(result.errors).length > 0) return;
	throw new Error(
		`${binding}: an invalid source produced no fatal diagnostic — oxc's severity vocabulary has ` +
			`moved, so every file would count as parsed and this row's coverage would be fabricated. ` +
			`See \`oxc_fatal_errors\` in lib/oxc.ts.`
	);
}

/**
 * The pinned layout targets, hoisted out of the per-call path: the timed loop
 * formats one file per call, so building a fresh options object per call is
 * harness-side allocation charged to this row.
 *
 * Matching prettier/tsv (printWidth 100, tabs, single quotes, no trailing commas)
 * is what makes every format row do the same layout work. oxfmt's own printWidth
 * default is already 100 — pinned anyway so a future default change can't silently
 * skew the rows; singleQuote (default false), useTabs and trailingComma differ for
 * real. `init` proves the three that differ LAND, because oxfmt ignores an
 * unrecognized option key silently; a behavioral probe cannot falsify printWidth
 * here precisely because 100 is already the default, which is also why it stays
 * pinned — that assertion still sees a changed default (`lib/format_config_probe.ts`).
 *
 * Frozen, and `svelte` gets its own bag rather than a mutated copy: the shared
 * object outlives the call now, so the `options.svelte = {}` this replaced would
 * have leaked the experimental path into every later language.
 */
const OXFMT_OPTIONS: Readonly<OxfmtFormatOptions> = Object.freeze({
	useTabs: true,
	printWidth: 100,
	singleQuote: true,
	trailingComma: 'none'
});

/** `OXFMT_OPTIONS` plus the key gating oxfmt's experimental `.svelte` handling. */
const OXFMT_SVELTE_OPTIONS: Readonly<OxfmtFormatOptions> = Object.freeze({
	...OXFMT_OPTIONS,
	svelte: {}
});

/**
 * OXC implementation using oxc-parser and oxfmt.
 *
 * Supports:
 * - Parse: TypeScript, JS (NOT Svelte, NOT CSS)
 * - Format: TypeScript, JS, CSS, Svelte (Svelte is experimental, expect partial coverage)
 */
export class OxcImplementation extends BaseImplementation {
	readonly versions: OxcVersions;
	private _parser: OxcParserModule | null = null;
	private _formatter: OxfmtModule | null = null;

	constructor(versions: OxcVersions) {
		super();
		this.versions = versions;
	}

	async init(): Promise<void> {
		const [parser_mod, formatter_mod] = await Promise.all([import('oxc-parser'), import('oxfmt')]);

		this._parser = parser_mod as OxcParserModule;
		this._formatter = formatter_mod as OxfmtModule;

		// Neither half of this impl reports a broken assumption on its own: oxc says
		// nothing about its diagnostic vocabulary, and oxfmt accepts an unrecognized
		// option key silently. Both are consumed through a cast, so prove them.
		//
		// ⚠️ One impl, two tools: either probe failing takes BOTH halves' rows down
		// (the parse rows and the format rows), since the registry's unit of absence
		// is the impl, not the row. Over-broad, and deliberately so — the alternative
		// is a second registry entry per binding — and disclosed rather than silent:
		// `unavailable[].rows` names every row the failure removed.
		assert_oxc_rejects_invalid(this._parser, 'oxc-parser');
		// Per LANGUAGE, and through `format_async` rather than the raw module call:
		// one options bag drives all three here (unlike biome's per-language sections),
		// so the extra two are corroboration — but they run the exact call the timed
		// row makes, which grades oxfmt's own diagnostics through `oxc_fatal_errors`,
		// and the svelte pass is the standing proof that the pins reach oxfmt's
		// bundled-prettier fallback (docs/benchmarks.md §Fairness caveats asserts it).
		for (const language of this.format_languages) {
			assert_format_config_landed(
				'oxfmt',
				language,
				await this.format_async(FORMAT_CONFIG_PROBES[language], language)
			);
		}
	}

	readonly parse_languages: ReadonlyArray<Language> = ['typescript'];
	readonly format_languages = LANGUAGES;

	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		if (!this._parser) throw new Error('OXC parser not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`OXC parser does not support ${language}`);
		}

		// A test262 goal pins oxc's `sourceType` so it's scored at the declared
		// goal like tsv/acorn, instead of oxc's filename-based inference.
		const options = goal ? { sourceType: goal } : undefined;
		const result = this._parser.parseSync(`file${LANGUAGE_EXTENSIONS[language]}`, source, options);

		// Read `errors` once into a local: the WASI sibling's getter is consume-once
		// (see oxc_wasm.ts / benches/js/CLAUDE.md §Known Issues); the native package
		// caches today, but the single-read form costs nothing and can't rot. Only
		// FATAL entries are a rejection — see `oxc_fatal_errors`.
		const errors = oxc_fatal_errors(result.errors);
		if (errors.length > 0) {
			throw new Error(`Parse errors: ${JSON.stringify(errors)}`);
		}

		// Accessing `.program` runs the package's `wrap()` getter, which `JSON.parse`s
		// the Rust-serialized AST — a full eager materialization (matching `tsv-json`,
		// so the `oxc-parser` row is apples-to-apples with it). There is deliberately no
		// lazy variant: oxc's `experimentalLazy` raw transfer is setup-dominated
		// (~1.7ms/call on Node, ~2.1ms on Deno, vs ~0.7ms eager + ~0.16ms parse-only) —
		// it eagerly copies the whole AST transfer buffer, so it measures buffer setup,
		// not parse speed, in any runtime. See `docs/benchmarks.md` §Fairness caveats.
		return result.program;
	}

	format(_source: string, _language: Language): string {
		// oxfmt is async, so we can't implement sync format
		throw new Error('OXC formatter is async-only, use format_async');
	}

	async format_async(source: string, language: Language): Promise<string> {
		if (!this._formatter) throw new Error('OXC formatter not initialized');
		if (!this.supports_format_language(language)) {
			throw new Error(`OXC formatter does not support ${language}`);
		}

		// The pinned layout targets (see `OXFMT_OPTIONS`). oxfmt gates .svelte handling
		// behind the `svelte` config key (experimental as of 0.49), which is the only
		// per-language difference, so it has its own hoisted bag.
		const options = language === 'svelte' ? OXFMT_SVELTE_OPTIONS : OXFMT_OPTIONS;

		const result = await this._formatter.format(
			`file${LANGUAGE_EXTENSIONS[language]}`,
			source,
			options
		);

		// Only FATAL diagnostics are a refusal to format — a warned-about file oxfmt
		// still formatted must not read as a skip. See `oxc_fatal_errors`.
		const errors = oxc_fatal_errors(result.errors);
		if (errors.length > 0) {
			throw new Error(`Format errors: ${JSON.stringify(errors)}`);
		}

		return result.code;
	}

	dispose(): void {
		this._parser = null;
		this._formatter = null;
	}
}

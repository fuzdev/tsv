/**
 * yuku-parser implementation wrapper — both bindings.
 *
 * yuku is a JS/TS parser written in Zig, shipped as two npm packages over one
 * engine: `yuku-parser` (N-API) and `@yuku-parser/wasm`. Parse only — it has no
 * formatter, no Svelte, and no CSS, so it contributes rows to `parse/typescript`
 * alone, exactly where oxc-parser sits.
 *
 * **One class drives both bindings.** They expose the identical module surface and
 * carry the identical hazards, so a per-binding wrapper would be a copy free to
 * drift — which is precisely how the oxc WASI binding once fabricated a 100%
 * coverage row (CLAUDE.md §Known Issues). Where oxc needs two files because its two
 * packages genuinely differ (`oxc.ts` / `oxc_wasm.ts` — different entry points, a
 * consume-once getter on one side), yuku needs one.
 *
 * The wasm package is driven through raw `WebAssembly` + manual linear-memory
 * copies — no wasm-bindgen, no WASI — so unlike the oxc WASM binding one entry
 * point works under all three runtimes. It instantiates at import time (top-level
 * `await`), which puts its setup in `init()` where every other impl's setup lives.
 * Its parse path encodes the source into linear memory, calls in, then copies the
 * result buffer back out — a boundary tax of the same shape `tsv_wasm` pays, and
 * the reason that row belongs beside `tsv_wasm-json` and `oxc-parser-wasm` rather
 * than beside the native ones.
 */

import { BaseImplementation, type Language, type ParseGoal } from './types.ts';
import type { YukuVersions } from './versions.ts';

/** A yuku diagnostic (the subset the bench reads). */
interface YukuDiagnostic {
	severity: 'error' | 'warning' | 'hint' | 'info';
	message: string;
}

/**
 * yuku's `ParseResult`. Every property is a LAZY memoized getter over the binary
 * buffer the Zig side returned — the types are written as plain fields upstream,
 * but reading one is what performs the decode. See `parse_yuku`.
 */
interface YukuParseResult {
	program: unknown;
	comments: unknown[];
	diagnostics: YukuDiagnostic[];
}

/** yuku's parse options (the subset the bench pins). */
interface YukuParseOptions {
	sourceType?: 'script' | 'module' | 'commonjs';
	lang?: 'js' | 'ts' | 'jsx' | 'tsx' | 'dts';
	preserveParens?: boolean;
	semanticErrors?: boolean;
	attachComments?: boolean;
}

/** The module surface both yuku packages expose (identical across the two). */
interface YukuParserModule {
	parse: (source: string, options?: YukuParseOptions) => YukuParseResult;
}

/** The two npm packages over one engine: bench row name → module specifier. */
const YUKU_BINDINGS = {
	'yuku-parser': 'yuku-parser',
	'yuku-parser-wasm': '@yuku-parser/wasm'
} as const;

/**
 * Which yuku binding a `YukuImplementation` drives — also its bench row name,
 * which keeps the shipped package name (`yuku-parser`) rather than shortening to
 * a bare tool name.
 */
export type YukuBinding = keyof typeof YUKU_BINDINGS;

/**
 * The pinned option set, shared by both bindings.
 *
 * Every value is set explicitly even where it matches yuku's current default, so
 * an upstream default change can't silently skew the rows — the same rule the
 * formatter rows follow for `printWidth`/`trailingComma` (CLAUDE.md §Fairness
 * Caveats). Each choice is a payload or work match against the opponents:
 *
 * - `sourceType: 'module'` — yuku's default too, and the goal tsv/acorn parse the
 *   perf corpus at. `parse_yuku` overrides it per file when the harness threads a
 *   test262 goal; pinning it here is what makes that override the only variable.
 * - `lang: 'ts'` — the corpus collapses `.js` and `.ts` into one `typescript`
 *   Language because tsv parses both through its TS path, and the bench hands oxc
 *   a synthetic `file.ts`. Parsing everything as TS matches both.
 * - `preserveParens: true` — yuku's and oxc's shared default (acorn, and so tsv,
 *   effectively parses with it off). Measured on this corpus it is immaterial:
 *   7–14 extra nodes out of ~5,600, a timing delta inside the noise floor. Pinned
 *   to oxc's value so the two span-only rows stay like-for-like rather than
 *   re-baselining oxc's committed numbers over a rounding error.
 * - `semanticErrors: false` — oxc's default too. Enabling it buys a second AST
 *   pass no opponent pays for.
 * - `attachComments: false` — payload match: neither oxc's `.program` nor tsv's
 *   wire AST carries comments (verified — tsv's wire `Program` has no `comments`
 *   key), so hanging them on nodes would be work the other rows never do.
 */
const PARSE_OPTIONS: YukuParseOptions = {
	sourceType: 'module',
	lang: 'ts',
	preserveParens: true,
	semanticErrors: false,
	attachComments: false
};

/**
 * Drive a yuku binding for one file, applying the two corrections that make the
 * row honest.
 *
 * ⚠ **`parse()` is LAZY.** It returns memoized getters over the binary buffer the
 * Zig side produced; the JS AST is built only when `.program` is read. Forcing it
 * is a large measured cost (the per-binding figures live in CLAUDE.md §Fairness
 * Caveats) — so a row that called `parse()` and discarded the result would report
 * a throughput for a tree nobody built, and would not be measuring the same
 * deliverable as `oxc-parser` (whose `.program` getter `JSON.parse`s) or
 * `tsv-json`. **Returning `result.program` is what forces the decode: never
 * "simplify" it to `return result`.**
 *
 * ⚠ **The parser is ERROR-TOLERANT — it never throws.** An invalid file yields an
 * empty AST plus `diagnostics`, so without reading them every file would count as
 * accepted and the coverage row would read 100% no matter what. That is exactly the
 * failure the oxc WASI binding's consume-once `errors` getter produced (CLAUDE.md
 * §Known Issues). Only `severity: 'error'` rejects — a warning/hint/info diagnostic
 * is not a parse failure, and treating one as such would under-report coverage.
 *
 * The error gate is a `some()` rather than a `filter()` so the timed happy path
 * allocates nothing the `oxc-parser` row doesn't also allocate (`oxc.ts` gates on
 * `errors.length`); the message array is built only once a file has already failed.
 */
function parse_yuku(
	parser: YukuParserModule,
	source: string,
	goal: ParseGoal | undefined
): unknown {
	// A test262 goal overrides the pinned `module` so yuku is scored at the goal the
	// test declares, like tsv/acorn/oxc — otherwise a script-goal `await`-identifier
	// test reads as a failure.
	const result = parser.parse(
		source,
		goal ? { ...PARSE_OPTIONS, sourceType: goal } : PARSE_OPTIONS
	);

	const diagnostics = result.diagnostics;
	if (diagnostics.some((d) => d.severity === 'error')) {
		const messages = diagnostics.filter((d) => d.severity === 'error').map((d) => d.message);
		throw new Error(`Parse errors: ${JSON.stringify(messages)}`);
	}

	// Forces the lazy decode — the full JS AST, matching `oxc-parser` and
	// `tsv-json`. See the laziness warning above before touching this line.
	return result.program;
}

/**
 * Assert at init that the pinned options actually REACH the parser, and fail the
 * impl loudly if they don't.
 *
 * The module is consumed through a cast (the house pattern — see `oxc.ts`), so an
 * upstream rename would leave our value inert and silently apply yuku's default
 * instead: the config-vs-engine conflation the fairness rules exist to prevent, and
 * the same hazard `dprint.ts` guards by asserting `getConfigDiagnostics()` is empty.
 * yuku has no diagnostic channel for an unrecognized option key, so the check is
 * behavioral instead.
 *
 * Only the two options whose loss would be SILENT are probed. `preserveParens`,
 * `semanticErrors`, and `attachComments` are pinned to values that match yuku's
 * own defaults, so a rename there is a no-op by construction:
 *
 * - `lang` — a lost `'ts'` falls back to `'js'`, which would reject the TypeScript
 *   corpus. Loud on its own, but probed anyway so the failure names its cause
 *   instead of surfacing as a mysterious 0% coverage row.
 * - `sourceType` — a lost goal falls back to `'module'`, which would mis-score the
 *   conformance surface's script-goal test262 files against every other tool.
 *
 * Both probes use `await`-as-an-identifier, which is the axis the harness threads
 * the goal FOR (see `SourceFile.goal`) and the one yuku moves: `var await` parses
 * at `script` and errors at `module`. The pair is asymmetric on purpose, and the
 * two prove different things: the `module` probe pins the pinned-and-default goal
 * (a silent upstream flip to `script` would otherwise change what every
 * perf-corpus file is parsed as, since `module` is also what yuku falls back to),
 * while the `script` probe is the only one that can catch a RENAME — it is the one
 * that proves an explicitly-passed `sourceType` reaches the parser at all.
 *
 * Neither probes module syntax, because yuku's script goal is **permissive** about
 * it — `import`, `export`, and `import.meta` all parse cleanly at
 * `sourceType: 'script'`, where tsv and acorn make them syntax errors. That is a
 * real parser difference, not a dropped option (`Program.sourceType` reflects the
 * goal either way), and it can only widen what yuku accepts on a script-goal file.
 * It costs the coverage surface nothing — a script-goal test262 positive contains
 * no module syntax by definition — but it means a yuku script-goal accept is a
 * weaker claim than a tsv one.
 */
function assert_options_land(parser: YukuParserModule, binding: YukuBinding): void {
	const fail = (option: string, detail: string): never => {
		throw new Error(
			`${binding}: option '${option}' did not land — ${detail}, so the pinned options are not ` +
				`reaching the parser (upstream rename?)`
		);
	};
	const rejects = (source: string, options: YukuParseOptions): boolean =>
		parser.parse(source, options).diagnostics.some((d) => d.severity === 'error');

	if (rejects('const enum E { A }\ntype T = string;\nlet x: T;', PARSE_OPTIONS)) {
		fail('lang', "TypeScript-only syntax was rejected under `lang: 'ts'`");
	}

	const await_binding = 'var await = 1;';
	if (!rejects(await_binding, PARSE_OPTIONS)) {
		fail('sourceType', '`var await` parsed cleanly at the pinned `module` goal');
	}
	if (rejects(await_binding, { ...PARSE_OPTIONS, sourceType: 'script' })) {
		fail('sourceType', "`var await` was rejected at `sourceType: 'script'`");
	}
}

/**
 * yuku-parser, driving either of its two bindings — the N-API row (the same
 * binding mechanism `oxc-parser` uses, under all three runtimes) or the WASM row.
 *
 * Supports:
 * - Parse: TypeScript, JS (NOT Svelte, NOT CSS)
 * - Format: None (yuku ships no formatter)
 */
export class YukuImplementation extends BaseImplementation {
	readonly name: YukuBinding;
	readonly versions: YukuVersions;
	private _parser: YukuParserModule | null = null;

	readonly parse_languages: ReadonlyArray<Language> = ['typescript'];
	/** yuku ships no formatter. */
	readonly format_languages: ReadonlyArray<Language> = [];

	constructor(name: YukuBinding, versions: YukuVersions) {
		super();
		this.name = name;
		this.versions = versions;
	}

	/**
	 * The version of THIS binding's package. The two ship in lockstep upstream, but
	 * each is reported from its own package so a skewed local install is visible in
	 * the report rather than implied.
	 */
	get version(): string {
		return this.name === 'yuku-parser' ? this.versions.parser : this.versions.wasm;
	}

	async init(): Promise<void> {
		const parser = (await import(YUKU_BINDINGS[this.name])) as YukuParserModule;
		assert_options_land(parser, this.name);
		this._parser = parser;
	}

	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		if (!this._parser) throw new Error(`${this.name} not initialized`);
		if (!this.supports_parse_language(language)) {
			throw new Error(`${this.name} does not support ${language}`);
		}
		return parse_yuku(this._parser, source, goal);
	}

	format(_source: string, _language: Language): string {
		throw new Error('yuku ships no formatter');
	}

	dispose(): void {
		this._parser = null;
	}
}

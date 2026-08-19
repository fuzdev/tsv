/**
 * TypeScript compiler (`tsc`) implementation wrapper — parse only.
 *
 * The language's own parser, and the one row here that is a *definition* rather
 * than an implementation: where acorn-typescript is tsv's drop-in target and oxc
 * is a peer, tsc decides what TypeScript syntax IS. It contributes a row to
 * `parse/typescript` alone (no formatter — tsc has no printer for source text, and
 * no Svelte/CSS).
 *
 * ⚠ **tsc's parser is ERROR-RECOVERING, so "accept" needs its own definition.**
 * `createSourceFile` never throws on bad syntax: it returns a `SourceFile` with the
 * error nodes it recovered plus a `parseDiagnostics` array. A row that scored
 * "didn't throw" would therefore report a fabricated 100% coverage — the same shape
 * as the oxc WASI consume-once bug (CLAUDE.md §Known Issues), reached by a
 * different road. So this wrapper reads `parseDiagnostics` and throws when it is
 * non-empty, which is what makes the row comparable to every throwing parser beside
 * it. `parseDiagnostics` is internal (absent from the public `SourceFile` type), so
 * it is read through a declared shape and a missing array is treated as a HARD
 * error rather than as "no diagnostics" — an upstream rename must not silently
 * restore the fabricated-100% row.
 *
 * **tsc INFERS the parse goal, so the `goal` argument is deliberately ignored.**
 * Every other parser here takes `sourceType` as an input; tsc derives it from the
 * file (`externalModuleIndicator` — a top-level `import`/`export` makes it a
 * module) and there is no override. Passing the conformance corpus's declared goal
 * would have nowhere to go. This is not a fairness hole: tsc's inference agrees
 * with the declared goal by construction on the tsc corpus (the harvest reads the
 * goal *from* tsc), and on test262 it is tsc applying its own rule, which is the
 * behavior a tsc row is supposed to show.
 *
 * **`.js` corpus files are parsed as TS**, matching the `oxc` row (which likewise
 * hands its parser a fixed `file.ts`): the `typescript` bench group is one group,
 * and per-file script-kind routing would make two rows measure different parsers on
 * the same file set. TypeScript is a superset for everything in scope here; the
 * places it isn't (annex-B HTML comments, legacy octals) are sloppy-mode-only and
 * excluded from test262's graded positives.
 */

import { BaseImplementation, type Language, type ParseGoal } from './types.ts';

/** One entry of tsc's internal parse-diagnostics array. */
export interface TscParseDiagnostic {
	code: number;
	messageText: unknown;
}

/**
 * The internal parse-diagnostics field. `ts.SourceFile` declares it `@internal`,
 * so it is absent from the published type — declared here rather than reached for
 * with an `any`, so the read has a name and the hard-error path below has a shape
 * to check.
 */
interface SourceFileWithParseDiagnostics {
	parseDiagnostics?: ReadonlyArray<TscParseDiagnostic>;
}

/** The `typescript` module surface this wrapper uses. */
export interface TypeScriptModule {
	version: string;
	ScriptTarget: { ESNext: number };
	ScriptKind: { TS: number };
	createSourceFile: (
		file_name: string,
		source_text: string,
		options: { languageVersion: number },
		set_parent_nodes: boolean,
		script_kind: number
	) => unknown;
	flattenDiagnosticMessageText: (text: unknown, newline: string) => string;
}

/**
 * Load the `typescript` package. The package ships a CJS default export and both
 * interop shapes appear depending on runtime + resolution mode, so the
 * normalization lives here once — every consumer (this row, the tsc-corpus
 * harvest) gets the same module object.
 */
export async function load_typescript(): Promise<TypeScriptModule> {
	const mod = await import('typescript');
	const normalized = (mod as { default?: TypeScriptModule }).default ?? mod;
	return normalized as unknown as TypeScriptModule;
}

/** What {@link tsc_parse} produces: the parse product plus tsc's verdict on it. */
export interface TscParseResult {
	/** tsc's `SourceFile` — error nodes and all, since its parser recovers. */
	source_file: unknown;
	/**
	 * The parse diagnostics; empty means tsc's parser accepted. `null` means the
	 * internal field is GONE (upstream rename) — never "no diagnostics", since with
	 * the field missing every input would read as accepted. No caller may pass on a
	 * `null`.
	 */
	diagnostics: ReadonlyArray<TscParseDiagnostic> | null;
}

/**
 * Parse `source` with tsc — the one place the parse configuration lives, so the
 * bench row and the harvest that grades its corpus can't drift apart on target,
 * script kind, or how a rejection is detected.
 *
 * `file_name` stays the caller's, since the two legitimately differ: the bench row
 * hands every file a fixed `file.ts` (one group, one parser — see the module doc),
 * while the harvest passes the real path.
 */
export function tsc_parse(ts: TypeScriptModule, file_name: string, source: string): TscParseResult {
	const source_file = ts.createSourceFile(
		file_name,
		source,
		{ languageVersion: ts.ScriptTarget.ESNext },
		false,
		ts.ScriptKind.TS
	);
	return {
		source_file,
		diagnostics: (source_file as SourceFileWithParseDiagnostics).parseDiagnostics ?? null
	};
}

export class TscImplementation extends BaseImplementation {
	/** Parse only, TypeScript/JS only — tsc has no source printer and no Svelte/CSS. */
	readonly parse_languages: ReadonlyArray<Language> = ['typescript'];
	readonly format_languages: ReadonlyArray<Language> = [];

	private _ts: TypeScriptModule | null = null;

	async init(): Promise<void> {
		this._ts = await load_typescript();
	}

	/** `goal` is accepted for interface parity and ignored — see the module doc. */
	parse(source: string, language: Language, _goal?: ParseGoal): unknown {
		if (!this._ts) throw new Error('tsc not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`tsc does not support ${language}`);
		}
		const ts = this._ts;
		const { source_file, diagnostics } = tsc_parse(ts, 'file.ts', source);
		if (!diagnostics) {
			throw new Error(
				'tsc: SourceFile has no `parseDiagnostics` — the internal field this row reads to ' +
					'detect a rejection is gone (upstream rename?). Refusing to score the file as ' +
					'parsed: every input would read as accepted.'
			);
		}
		if (diagnostics.length > 0) {
			const first = diagnostics[0];
			throw new Error(
				`Parse error TS${first.code}: ${ts.flattenDiagnosticMessageText(first.messageText, ' ')}`
			);
		}
		return source_file;
	}

	dispose(): void {
		this._ts = null;
	}
}

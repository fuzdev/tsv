/**
 * OXC WASM implementation wrapper (oxc-parser via wasm32-wasi)
 *
 * Loads @oxc-parser/binding-wasm32-wasi via the entry each runtime can handle
 * (see `init`): Deno and Bun get the fetch-based browser entry, Node gets the
 * default `node:wasi` entry — so the oxc-parser-wasm row runs under all three.
 *
 * Supports: Parse only (TypeScript, JS). No formatting (oxfmt has no WASM variant).
 */

import { BaseImplementation, type Language, LANGUAGE_EXTENSIONS, type ParseGoal } from './types.ts';
import type { OxcVersions } from './versions.ts';
import { assert_oxc_rejects_invalid, type OxcDiagnostic, oxc_fatal_errors } from './oxc.ts';
import { current_runtime } from './runtime.ts';

/** oxc-parser WASM module types (same API as native) */
interface OxcParserWasmModule {
	parseSync: (
		filename: string,
		source: string,
		options?: { sourceType?: 'script' | 'module' }
	) => { program: unknown; errors: OxcDiagnostic[] };
}

/** oxc-parser's own `{node, fixes}` deserializer (`src-js/wrap.js`, untyped). */
type JsonParseAst = (programJson: string) => unknown;

/**
 * OXC WASM implementation using @oxc-parser/binding-wasm32-wasi.
 *
 * Supports:
 * - Parse: TypeScript, JS (NOT Svelte, NOT CSS)
 * - Format: None (oxfmt has no WASM variant)
 */
export class OxcWasmImplementation extends BaseImplementation {
	readonly versions: OxcVersions;
	private _parser: OxcParserWasmModule | null = null;
	private _json_parse_ast: JsonParseAst | null = null;

	readonly parse_languages: ReadonlyArray<Language> = ['typescript'];
	/** oxfmt has no WASM variant. */
	readonly format_languages: ReadonlyArray<Language> = [];

	constructor(versions: OxcVersions) {
		super();
		this.versions = versions;
	}

	async init(): Promise<void> {
		// The WASI binding ships two entries: the default CJS entry (`parser.wasi.cjs`)
		// uses `node:wasi`, which only Node implements far enough to instantiate —
		// Deno ships no `node:wasi` at all, and Bun's `WASI` class has no
		// `initialize`. The explicit fetch-based browser entry
		// (`parser.wasi-browser.js`, `@napi-rs/wasm-runtime` + `WebAssembly`) is the
		// mirror image: Deno and Bun both load it, Node can't. So the runtime that
		// takes the default entry is Node alone, and the oxc-parser-wasm comparison
		// row is available on all three.
		const entry =
			current_runtime() === 'node'
				? '@oxc-parser/binding-wasm32-wasi'
				: '@oxc-parser/binding-wasm32-wasi/parser.wasi-browser.js';
		const mod = await import(entry);
		this._parser = mod as OxcParserWasmModule;
		// The native package's own deserializer (dependency-free ESM; the package has
		// no `exports` map, so the deep import resolves). Importing rather than
		// copying keeps the two rows' materialization identical by construction and
		// tracks the pinned oxc-parser version; a moved upstream path fails init loudly.
		const wrap = await import('oxc-parser/src-js/wrap.js');
		this._json_parse_ast = (wrap as { jsonParseAst: JsonParseAst }).jsonParseAst;

		// The same rejection probe the native binding runs, from the same module, so
		// the two bindings cannot come to disagree about what an oxc rejection IS.
		// That disagreement is exactly how this row broke once before — a different
		// cause (the consume-once `errors` getter above), the same shape.
		assert_oxc_rejects_invalid(this._parser, 'oxc-parser-wasm');
	}

	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		if (!this._parser) throw new Error('OXC WASM parser not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`OXC WASM parser does not support ${language}`);
		}

		const options = goal ? { sourceType: goal } : undefined;
		const result = this._parser.parseSync(`file${LANGUAGE_EXTENSIONS[language]}`, source, options);

		// Read `errors` exactly ONCE: on the WASI binding it's a CONSUME-ONCE getter —
		// the first access returns the real error array, every later access returns
		// `[]`. A double-access check (`result.errors && result.errors.length`) eats
		// the real array in the truthiness test and always sees length 0, silently
		// accepting every file (invalid input yields an empty Program with end 0) —
		// which fabricated a 100% conformance-coverage row. See
		// benches/js/CLAUDE.md §Known Issues. `oxc_fatal_errors` (shared with the
		// native binding) then decides which of those entries are a rejection.
		const errors = oxc_fatal_errors(result.errors);
		if (errors.length > 0) {
			throw new Error(`Parse errors: ${JSON.stringify(errors)}`);
		}

		// Unlike the native `oxc-parser` package (whose `index.js` `wrap()` runs
		// `jsonParseAst` on `.program` access), the WASI binding hands back `program`
		// as the raw JSON string the Rust side serialized — it never deserializes.
		// Run it through the SAME `jsonParseAst` (imported in `init`) so
		// `oxc-parser-wasm` materializes the identical JS AST native `oxc-parser`
		// does — including the `fixes` pass that turns bigint/regex `Literal.value`s
		// into real `BigInt`/`RegExp` instances (apples-to-apples timing AND shape;
		// a bare `JSON.parse(...).node` would skip that work and leave the two rows'
		// ASTs disagreeing on those literals).
		const program = result.program;
		if (typeof program !== 'string') return program;
		if (!this._json_parse_ast) throw new Error('OXC WASM jsonParseAst not initialized');
		return this._json_parse_ast(program);
	}

	format(_source: string, _language: Language): string {
		throw new Error('OXC WASM does not support formatting (oxfmt has no WASM variant)');
	}

	dispose(): void {
		this._parser = null;
		this._json_parse_ast = null;
	}
}

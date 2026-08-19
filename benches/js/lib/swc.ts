/**
 * swc implementation wrapper (`@swc/core`, N-API)
 *
 * The most widely deployed Rust TypeScript/JS parser, and the one alternative
 * engine the `parse/typescript` group was missing while carrying two bindings each
 * of oxc and yuku. Parse-only: swc is a whole compiler, but nothing else here is
 * comparable work (its formatter does not exist; its transforms are a different
 * job than formatting).
 *
 * **Not payload-matched — its AST is its own dialect.** Root is `Module` (not
 * `Program`), positions ride a `span` rather than `loc`/`range`, and node kinds are
 * `Ts`-prefixed (`TsTypeAliasDeclaration`, `TsFunctionType`). So it belongs with
 * the oxc-class disclosure: a ratio against `tsv-json` compares two different
 * products, not two spellings of one. Unlike oxc/yuku it is NOT span-only-padded
 * either, so it is not an opponent for the `no-locations` curated lines.
 *
 * Two properties decide how it must be driven:
 *
 * - **Config must LAND.** swc defaults `decorators: false` in its TS config, and
 *   silently — a decorated class then fails with a bare "Expression expected"
 *   that reads as a parser limitation. That alone produced a phantom rejection on
 *   real corpus code (`fuz_code`'s `sample_complex.ts`, `@some_decorator` on a
 *   class). `init()` asserts the option takes effect by parsing a decorator, the
 *   same "prove the config landed" discipline `lib/dprint.ts` applies to its
 *   config diagnostics.
 * - **It THROWS on invalid input**, which is the clean case — no correction needed
 *   like yuku's error-tolerance (never throws, returns diagnostics) or tsc's
 *   error recovery (`parseDiagnostics.length === 0`). An accept here is an accept.
 *
 * **Goal axis** (the conformance surface's test262 files declare one): swc spells
 * it `isModule`, not `script`. `isModule: false` accepts a bare `await` as an
 * identifier and rejects `import`; the default does the reverse — verified in both
 * directions, so the flag genuinely varies the goal rather than merely being
 * accepted. Without this a script-goal `await`-identifier test would be scored as
 * a module-goal failure (the same trap documented for the other parsers in
 * docs/benchmarks.md §Fairness caveats).
 *
 * The real-corpus files it rejects are catalogued in `lib/perf_omit.ts`; all are
 * `.d.ts` files already tolerated there for other tools.
 */

import { createRequire } from 'node:module';
import { BaseImplementation, type Language, type ParseGoal } from './types.ts';
import type { SwcVersions } from './versions.ts';

interface SwcParseOptions {
	syntax: 'typescript' | 'ecmascript';
	target: string;
	decorators: boolean;
	isModule: boolean;
}

interface SwcModule {
	parseSync: (source: string, options: SwcParseOptions) => { type: string };
}

/**
 * swc's parser.
 *
 * Supports:
 * - Parse: TypeScript, JS
 * - Format: unsupported — swc ships no formatter.
 */
export class SwcImplementation extends BaseImplementation {
	readonly versions: SwcVersions;
	private _swc: SwcModule | null = null;

	readonly parse_languages: ReadonlyArray<Language> = ['typescript'];
	/** swc has no formatter. */
	readonly format_languages: ReadonlyArray<Language> = [];

	constructor(versions: SwcVersions) {
		super();
		this.versions = versions;
	}

	// deno-lint-ignore require-await
	async init(): Promise<void> {
		// `require` for the same reason as lib/rsvelte_parse.ts — a CJS entry resolving
		// a platform-specific `.node`, whose ESM-namespace shape the three runtimes
		// don't agree on. Loading here rather than statically is what makes a missing
		// platform binding a skipped impl instead of a dead registry.
		const require = createRequire(import.meta.url);
		const swc = require('@swc/core') as SwcModule;
		if (typeof swc.parseSync !== 'function') {
			throw new Error('@swc/core exposes no parseSync()');
		}

		// Assert `decorators: true` actually reaches the parser. swc accepts unknown
		// option keys silently, so a renamed key in a future version would leave
		// decorators off and turn every decorated file into a fabricated rejection —
		// scoring tsv's corpus against a misconfigured opponent.
		swc.parseSync('@dec class C {}', this._options('module'));

		// Assert the GOAL axis still swings, in both directions — the same silent-key
		// hazard, on the option that decides how the conformance surface SCORES this
		// row. At script goal `await` is an ordinary identifier and `import` is an
		// error; at module goal the reverse. An inert `isModule` (which is what
		// `script: true` already is here) would mark every script-goal test262 file a
		// module-goal failure and read as a parser gap.
		const accepts = (source: string, goal: ParseGoal): boolean => {
			try {
				swc.parseSync(source, this._options(goal));
				return true;
			} catch {
				return false;
			}
		};
		if (
			!accepts('var await = 1;', 'script') ||
			accepts('var await = 1;', 'module') ||
			accepts('import x from "y";', 'script') ||
			!accepts('import x from "y";', 'module')
		) {
			throw new Error("swc's `isModule` no longer selects the parse goal");
		}

		this._swc = swc;
	}

	/**
	 * `syntax` is always `typescript` — the superset for everything in scope, and
	 * what every call site here wants (see `parse`). The option exists on swc's
	 * side; pinning it in one place keeps the two call sites from drifting.
	 */
	private _options(goal: ParseGoal): SwcParseOptions {
		return {
			syntax: 'typescript',
			target: 'esnext',
			decorators: true,
			// `isModule` is the goal switch (`script: true` is NOT it — verified
			// inert). See the module doc.
			isModule: goal !== 'script'
		};
	}

	parse(source: string, language: Language, goal: ParseGoal = 'module'): unknown {
		if (!this._swc) throw new Error('swc not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`swc does not support ${language}`);
		}
		// The corpus folds `.js` into the `typescript` Language and the impl call
		// carries no real path, so every file is parsed as TypeScript — the same
		// synthetic treatment lib/dprint.ts and lib/biome.ts give the language. TS is
		// the superset for everything in scope here (no JSX: tsv rejects it by
		// design, so the corpus carries none).
		return this._swc.parseSync(source, this._options(goal));
	}

	dispose(): void {
		this._swc = null;
	}
}

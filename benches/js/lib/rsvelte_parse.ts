/**
 * rsvelte parse implementation wrapper (N-API addon)
 *
 * The **only third-party engine on the `parse/svelte` surface** — the rest of that
 * group is `svelte/compiler` (the oracle) and tsv's own variants, so without this
 * row tsv is measured against its own reference and nothing else. rsvelte is the
 * other Rust-native Svelte toolchain, and its parse binding claims the same
 * drop-in contract tsv does, which makes it a conformance peer as well as a speed
 * row.
 *
 * **Why this package and not `@rsvelte/compiler`.** The scope publishes `fmt`,
 * `lint`, `language-server`, `svelte-check`, `compiler`, and this addon; only this
 * one exposes an in-process parse with options. `@rsvelte/compiler` is a WASM
 * bundle (its files are named `rsvelte_lint*`) whose `parse_svelte` takes no
 * options and returns **pretty-printed** JSON. The indentation happens inside the
 * wasm before the string crosses the boundary, so there is no way to opt out: a
 * timed row would rank JSON formatting and a fatter boundary crossing, not parse
 * work. It also reports an internal `version()` rather than the upstream Svelte
 * version, and drops `Line` comment nodes this binding keeps. So the
 * misleadingly-named addon is the honest artifact. (`lib/rsvelte.ts` — the
 * coverage-only *formatter* row — already cites this same package, for the
 * opposite reason: it has no format export.)
 *
 * **Mechanism-matched to `tsv-json`.** `parse()` returns the AST as a JSON string
 * that the caller `JSON.parse`s — exactly what tsv's FFI/WASM parse rows do — so
 * this is an apples-to-apples comparison rather than a disclosed approximation.
 * Its root keys are identical to `svelte/compiler`'s (`comments, css, end,
 * fragment, instance, js, options, start, type`) and its `VERSION` reports the
 * upstream Svelte it targets, which currently matches the harness's pin.
 *
 * **It parses the whole conformance Svelte corpus without a host fault** — stated
 * because that is precisely where yuku's N-API binding segfaults (`lib/yuku.ts`),
 * which cost that engine its row on this surface. Re-check on an rsvelte bump: a
 * native addon that aborts mid-preflight kills every run, for every tool.
 *
 * ⚠ **The reduced row is NOT payload-matched to tsv's `no-locations` wire**, which
 * is why it is named for the option it passes rather than for tsv's. tsv drops
 * per-node `loc` throughout; `skipExpressionLoc` drops only the nested `loc`
 * blocks on embedded JavaScript expressions and keeps top-level `start`/`end`, so
 * it reduces strictly less. Two different reductions — read the pair as "each
 * tool's own lighter wire", never as one payload measured twice.
 *
 * ⚠ `ParseOptions.skipCssAst` is documented upstream but is a **no-op** in 0.3.4
 * (byte-identical payload on every styled component tried, and `css` still
 * carries the full `StyleSheet` AST rather than the documented stub), so it
 * gets no row — it would silently duplicate the plain one. `parseEnvelope` (the
 * raw-transfer binary path) gets none either: it **drops `leadingComments`**
 * relative to `parse()`, so it is a lossier product, not a faster equal.
 */

import { createRequire } from 'node:module';
import { BaseImplementation, type Language } from './types.ts';
import type { RsvelteParseVersions } from './versions.ts';

/** The subset of the addon's surface this row drives. */
interface RsvelteNative {
	parse: (source: string, options?: { skipExpressionLoc?: boolean }) => string;
	VERSION: string;
}

/**
 * rsvelte's Svelte parser, via its N-API addon.
 *
 * Supports:
 * - Parse: Svelte only. The addon also exposes `compile`/`svelte2tsx`, and its
 *   `.ts`/`.css` handling is not a parser surface at all — nothing here needs it.
 * - Format: unsupported — this package has no format export (that is the separate
 *   `@rsvelte/fmt` CLI, whose row is coverage-only; see `lib/rsvelte.ts`).
 */
export class RsvelteParseImplementation extends BaseImplementation {
	readonly name = 'rsvelte-parse' as const;
	readonly versions: RsvelteParseVersions;
	private _native: RsvelteNative | null = null;

	readonly parse_languages: ReadonlyArray<Language> = ['svelte'];
	/** No format export in this package — see the module doc. */
	readonly format_languages: ReadonlyArray<Language> = [];

	constructor(versions: RsvelteParseVersions) {
		super();
		this.versions = versions;
	}

	/** The upstream Svelte version the addon targets, once initialized. */
	get upstream_svelte_version(): string | undefined {
		return this._native?.VERSION;
	}

	// deno-lint-ignore require-await
	async init(): Promise<void> {
		// `createRequire` rather than `await import()`: the addon is CJS whose entry
		// resolves a platform-specific `.node` at require time, and the three runtimes
		// disagree on what the ESM-over-CJS namespace looks like for that shape —
		// `require` is the one spelling all three agree on (verified under Deno, Node
		// and Bun). Loading in `init()` at all (rather than statically) is what keeps
		// a load failure inside `init_implementations`' per-impl try/catch, the same
		// posture as lib/biome.ts / lib/dprint.ts / lib/yuku.ts.
		const require = createRequire(import.meta.url);
		const native = require('@rsvelte/vite-plugin-svelte-native') as RsvelteNative;

		// Probe the surface rather than trusting the package: a present-but-broken
		// binding must fail as a broken setup, not read as an honest 0% coverage.
		if (typeof native.parse !== 'function') {
			throw new Error('@rsvelte/vite-plugin-svelte-native exposes no parse()');
		}

		// Assert `skipExpressionLoc` still REDUCES. This addon already ships one
		// documented option that silently does nothing (`skipCssAst` — see the module
		// doc), and an inert `skipExpressionLoc` would be invisible in exactly the
		// wrong way: the reduced row would keep parsing every file at a plausible
		// speed, and the report would publish two rows as if they measured different
		// wires. The probe needs an embedded expression with a type annotation, since
		// that is the only thing the option drops.
		const probe = '<script lang="ts">let n: number = 1;</script><p>{n + 1}</p>';
		const full = native.parse(probe);
		const reduced = native.parse(probe, { skipExpressionLoc: true });
		if (reduced.length >= full.length) {
			throw new Error(
				`rsvelte's skipExpressionLoc no longer reduces the payload (${full.length} → ` +
					`${reduced.length} bytes) — it has gone inert like skipCssAst, so the reduced row ` +
					`would silently duplicate the plain one`
			);
		}

		this._native = native;
	}

	parse(source: string, language: Language): unknown {
		if (!this._native) throw new Error('rsvelte parse not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`rsvelte parse does not support ${language}`);
		}
		// `parse()` hands back JSON; the `JSON.parse` is the caller's cost in the
		// real consumer too, and including it is what makes this mechanism-matched
		// to `tsv-json` (which pays the identical boundary + parse cost).
		return JSON.parse(this._native.parse(source));
	}

	/**
	 * The `skipExpressionLoc` wire — rsvelte's own lighter payload. Deliberately
	 * NOT named `parse_no_locations`: that interface hook means tsv's span-only
	 * wire, and this is a different reduction (see the module doc), so it stays a
	 * method of its own rather than borrowing a name that would assert a payload
	 * match the bytes don't support.
	 */
	parse_skip_expression_loc(source: string, language: Language): unknown {
		if (!this._native) throw new Error('rsvelte parse not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`rsvelte parse does not support ${language}`);
		}
		return JSON.parse(this._native.parse(source, { skipExpressionLoc: true }));
	}

	dispose(): void {
		this._native = null;
	}
}

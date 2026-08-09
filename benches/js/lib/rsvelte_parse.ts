/**
 * rsvelte parse implementation wrapper (N-API addon)
 *
 * The **first third-party engine on the `parse/svelte` surface**. Before this row
 * that group held only `svelte/compiler` (the oracle) and tsv's own variants — tsv
 * measured against its own reference and nothing else. rsvelte is the other
 * Rust-native Svelte toolchain, and its parse binding claims the same drop-in
 * contract tsv does, which makes it a conformance peer as well as a speed row.
 *
 * **Why this package and not `@rsvelte/compiler`.** The scope publishes `fmt`,
 * `lint`, `language-server`, `svelte-check`, `compiler`, and this addon; only this
 * one exposes an in-process parse with options. `@rsvelte/compiler` is a WASM
 * bundle (its files are named `rsvelte_lint*`) whose `parse_svelte` takes no
 * options and returns **pretty-printed** JSON — 267,010 bytes where this addon
 * emits 76,509 for the same component, 76,221 of it once compacted. The
 * indentation happens inside the wasm before the string crosses the boundary, so
 * there is no way to opt out: a timed row would rank JSON formatting and a 3.5x
 * larger boundary crossing, not parse work. It also reports an internal
 * `version()` (0.8.0) rather than the upstream Svelte version, and drops two
 * `Line` comment nodes this binding keeps. So the misleadingly-named addon is the
 * honest artifact. (`lib/rsvelte.ts` — the coverage-only *formatter* row — already
 * cites this same package, for the opposite reason: it has no format export.)
 *
 * **Mechanism-matched to `tsv-json`.** `parse()` returns the AST as a JSON string
 * that the caller `JSON.parse`s — exactly what tsv's FFI/WASM parse rows do — so
 * this is an apples-to-apples comparison rather than a disclosed approximation.
 * Its root keys are identical to `svelte/compiler`'s (`comments, css, end,
 * fragment, instance, js, options, start, type`) and its `VERSION` reports the
 * upstream Svelte it targets, which currently matches the harness's pin.
 *
 * ⚠ **The reduced row is NOT payload-matched to tsv's `no-locations` wire**, which
 * is why it is named for the option it passes rather than for tsv's. tsv drops
 * per-node `loc` throughout (~46% smaller); `skipExpressionLoc` drops only the
 * nested `loc` blocks on embedded JavaScript expressions and keeps top-level
 * `start`/`end` (-34% measured). Two different reductions — read the pair as
 * "each tool's own lighter wire", never as one payload measured twice.
 *
 * ⚠ `ParseOptions.skipCssAst` is documented upstream but is a **no-op** in 0.3.4
 * (byte-identical payload on a component with a 1,564-byte `<style>`, and `css`
 * still carries the full `StyleSheet` AST rather than the documented stub), so it
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

	async init(): Promise<void> {
		// `createRequire` rather than a static import: the addon is CJS with a
		// platform-specific `.node` binding, and requiring it lazily here keeps a
		// load failure inside `init_implementations`' per-impl try/catch (the same
		// posture as lib/biome.ts / lib/dprint.ts) instead of aborting the registry's
		// static import graph. Verified to load under all three runtimes.
		const require = createRequire(import.meta.url);
		const native = require('@rsvelte/vite-plugin-svelte-native') as RsvelteNative;

		// Probe the surface rather than trusting the package: a present-but-broken
		// binding must fail as a broken setup, not read as an honest 0% coverage.
		if (typeof native.parse !== 'function') {
			throw new Error('@rsvelte/vite-plugin-svelte-native exposes no parse()');
		}
		native.parse('<p>probe</p>');
		this._native = native;
		await Promise.resolve();
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

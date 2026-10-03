/**
 * Types for the line/column reconstruction helper (`locations.js`).
 *
 * Hand-written: these functions add `loc` to the span-only object graph every tsv parse
 * returns. See `locations.js` for the definition both the helper and the Rust `loc`
 * emitter (`tsv parse --locations`) implement.
 */

/** The document's language, which selects its line-terminator rule. */
export type LocationLanguage = 'typescript' | 'svelte' | 'css';

/** `reconstruct_locations`' options: the language, inferred from the AST root when omitted. */
export interface LocationOptions {
	/**
	 * The document's language. `typescript` counts ECMAScript LineTerminators and counts
	 * a leading BOM, as acorn does; `svelte` and `css` count LF alone — a Svelte
	 * document's `<script>`s and `<style>` included — and elide a leading BOM, as
	 * Svelte's `parse` and `parseCss` do. Omitted, it is read off the root (`Root`,
	 * `StyleSheetFile`, or a `Program` spanning the whole source); any other node — a
	 * subtree such as a Svelte `Fragment`, or a `<script>`'s `Program`, names no
	 * document — throws, so name the language to reconstruct one.
	 */
	language?: LocationLanguage | undefined;
}

/**
 * `create_locator` / `loc_of`'s options: the language is REQUIRED, since a bare source
 * or a lone node names no document (a missing or unknown one throws).
 */
export interface LocatorOptions {
	/** The document's language — see `LocationOptions.language`. */
	language: LocationLanguage;
}

/**
 * A reconstructed `loc` object (1-based line / 0-based UTF-16 column), matching
 * the loc-bearing wire's shape. The point type is inlined rather than named to
 * avoid colliding with `tsv_ast.d.ts`'s `Position` — both `.d.ts` files are
 * re-exported from the package root, and two star re-exports of the same name are
 * ambiguated away.
 */
export interface Loc {
	start: { line: number; column: number };
	end: { line: number; column: number };
}

/** A source-bound locator holding a prebuilt line-start table (`create_locator`). */
export interface Locator {
	/** Line/column for one node, or `null` if it has no numeric `start`/`end`. */
	loc_of(node: any): Loc | null;
	/**
	 * Add `loc` to every object in `ast` with numeric `start`/`end` — and `name_loc` to
	 * the Svelte elements, attributes, and directives that carry one — mutating in place;
	 * returns `ast`. A Svelte subtree reconstructs without the in-tag comments'
	 * `character` stamp, which reads the root's `comments` list.
	 */
	reconstruct<T>(ast: T): T;
}

/**
 * Build a locator that holds the source's line-start table for repeated lookups.
 * Prefer this over the bare helpers for heavy sparse use.
 */
export declare function create_locator(source: string, opts: LocatorOptions): Locator;

/**
 * Add a `loc` line/column object to every object of a span-only tree that carries
 * numeric `start`/`end`, derived from those offsets + `source` — plus the Svelte
 * `name_loc`. Mutates `ast` in place and returns it. The result deep-equals the Rust
 * emitter's loc-bearing wire of the same parse (`tsv parse --locations`), in every
 * language — and is what a parse with `{locations: true}` returns.
 *
 * @throws when `opts.language` is omitted and `ast` is not a parse's root (`Root`,
 *   `StyleSheetFile`, or a `Program` spanning the whole source), or names a language
 *   that is not one of the three.
 */
export declare function reconstruct_locations<T>(ast: T, source: string, opts?: LocationOptions): T;

/**
 * Line/column for a single node. Rebuilds the line-start table per call — reuse a
 * `create_locator` for more than a couple of lookups against one source.
 */
export declare function loc_of(node: any, source: string, opts: LocatorOptions): Loc | null;

/**
 * Types for the line/column reconstruction helper (`locations.js`).
 *
 * Hand-written: these functions add `loc` to the span-only object graph every tsv parse
 * returns. See `locations.js` for the definition both the helper and the Rust `loc`
 * emitter (`tsv parse --locations`) implement.
 *
 * The position types are `tsv_ast.d.ts`'s, IMPORTED and never re-exported: a package's
 * `index.d.ts` star-exports both files, so a name both exported would be ambiguated away
 * (TS2308). This file ships only beside `tsv_ast.d.ts`.
 */

import type { Position, SourceLocation } from './tsv_ast.js';

/** The document's language, which selects its line-terminator rule. */
export type LocationLanguage = 'typescript' | 'svelte' | 'css';

/** `reconstruct_locations`' options: the language, inferred from the AST root when omitted. */
export interface ReconstructLocationsOptions {
	/**
	 * The document's language. `typescript` counts ECMAScript LineTerminators and counts
	 * a leading BOM, as acorn does; `svelte` and `css` count LF alone — a Svelte
	 * document's `<script>`s and `<style>` included — and elide a leading BOM, as
	 * Svelte's `parse` and `parseCss` do. Omitted, it is read off the root (`Root`,
	 * `StyleSheetFile`, or a `Program` spanning the whole source); any other node — a
	 * subtree such as a Svelte `Fragment`, or a `<script>`'s `Program`, names no
	 * document — throws a `TypeError`, so name the language to reconstruct one.
	 */
	language?: LocationLanguage | undefined;
}

/**
 * `create_locator`'s options: the language is REQUIRED, since a bare source names no
 * document (a missing or unknown one throws a `TypeError`). The bag is read as every tsv
 * options bag is: a non-object or an unknown key throws too.
 */
export interface LocatorOptions {
	/** The document's language — see `ReconstructLocationsOptions.language`. */
	language: LocationLanguage;
}

/**
 * A source-bound locator holding a prebuilt line-start table (`create_locator`).
 *
 * Offsets are the wire's: UTF-16 units into the text the parse's offsets index — the
 * source itself for TypeScript, the source without a leading BOM for Svelte and CSS.
 */
export interface Locator {
	/**
	 * Line (1-based) and column (0-based, UTF-16 units) of one offset.
	 *
	 * @throws TypeError when `offset` is not a number
	 * @throws RangeError when `offset` is a number but not an integer from 0 to the indexed
	 *   text's length (the end of the text is a position)
	 */
	position_at(offset: number): Position;
	/**
	 * Line/column for one node's `start` and `end`, or `null` when either is not a number
	 * (`null` and `undefined` nodes included) — the same objects the whole-tree walk skips,
	 * since only an object carrying numeric `start`/`end` gets a `loc`.
	 *
	 * @throws RangeError when `start` and `end` are numbers but not a range of the indexed
	 *   text — an offset that is not an integer, one past its end, or `start` after `end`
	 */
	loc_of(
		node: { start?: number | undefined; end?: number | undefined } | null | undefined
	): SourceLocation | null;
	/**
	 * Add `loc` to every object in `ast` with numeric `start`/`end` — and `name_loc` to
	 * the Svelte elements, attributes, and directives that carry one — mutating in place;
	 * returns `ast`. A Svelte subtree reconstructs without the in-tag comments'
	 * `character` stamp, which reads the root's `comments` list. The walk checks no
	 * offset: it trusts `ast` to be a parse of the locator's source. `ast` must be acyclic —
	 * the parse's own tree, or a structured clone of it: the walk keeps no visited set, so a
	 * tree given back-pointers (a `parent` on each node) never finishes.
	 */
	reconstruct<T>(ast: T): T;
}

/**
 * Build a locator that holds the source's line-start table, so any number of lookups
 * against one source build it once.
 *
 * @throws TypeError when `source` is not a string, `options` is not an object or carries
 *   a key other than `language`, or `options.language` is missing or not one of the three
 */
export declare function create_locator(source: string, options: LocatorOptions): Locator;

/**
 * Add a `loc` line/column object to every object of a span-only tree that carries
 * numeric `start`/`end`, derived from those offsets + `source` — plus the Svelte
 * `name_loc`. Mutates `ast` in place and returns it. The result deep-equals the Rust
 * emitter's loc-bearing wire of the same parse (`tsv parse --locations`), in every
 * language — and is what a parse with `{locations: true}` returns. The walk checks no
 * offset: it trusts `ast` to be a parse of `source`. `ast` must be acyclic — the parse's own
 * tree, or a structured clone of it: a tree given back-pointers (a `parent` on each node)
 * never finishes.
 *
 * @throws TypeError when `source` is not a string, `options` is not an object or carries a
 *   key other than `language`, `options.language` is omitted and `ast` is not a parse's
 *   root (`Root`, `StyleSheetFile`, or a `Program` spanning the whole source), or it names
 *   a language that is not one of the three
 */
export declare function reconstruct_locations<T>(
	ast: T,
	source: string,
	options?: ReconstructLocationsOptions
): T;

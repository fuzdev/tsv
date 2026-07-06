/**
 * Types for the line/column reconstruction helper (`locations.js`).
 *
 * Hand-written: the span-only `no-locations` wire is untyped (`any`), and these
 * functions add `loc` to that same object graph. See `locations.js` for the exact
 * TypeScript/Svelte/CSS behavior.
 */

/** Language selector for the line-terminator rule. */
export type LocationLanguage = 'typescript' | 'svelte' | 'css';

/** Options shared by every entry point. */
export interface LocationOptions {
	/**
	 * Which line-terminator rule to apply. `typescript` (the default) uses
	 * ECMAScript LineTerminators; `svelte` uses LF-only; `css` makes
	 * `reconstruct` a no-op. `reconstruct_locations` infers this from the AST
	 * root when omitted.
	 */
	language?: LocationLanguage;
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
	/** Add `loc` to every node in `ast`, mutating in place; returns `ast`. */
	reconstruct(ast: any): any;
}

/**
 * Build a locator that holds the source's line-start table for repeated lookups.
 * Prefer this over the bare helpers for heavy sparse use.
 */
export declare function create_locator(source: string, opts?: LocationOptions): Locator;

/**
 * Add a `loc` line/column object to every node of a span-only wire, derived from
 * `start`/`end` + `source`. Mutates `ast` in place and returns it. Exact for
 * TypeScript, approximate for Svelte, a no-op for CSS.
 */
export declare function reconstruct_locations(ast: any, source: string, opts?: LocationOptions): any;

/**
 * Line/column for a single node. Rebuilds the line-start table per call — reuse a
 * `create_locator` for more than a couple of lookups against one source.
 */
export declare function loc_of(node: any, source: string, opts?: LocationOptions): Loc | null;

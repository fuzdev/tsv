/**
 * Line/column reconstruction for tsv's `no-locations` parse wire.
 *
 * The `parse_*_no_locations` exports emit a span-only AST: every node keeps its
 * `start`/`end` (UTF-16 code-unit offsets) but drops the per-node `loc`
 * (line/column) object (Svelte also drops `name_loc`). Line/column is a pure
 * function of an offset plus the source, so a consumer holding only the span-only
 * wire recovers `loc` on demand — no re-parse. This module is that reconstruction,
 * shipped so callers don't reimplement the line rules.
 *
 * Two entry points:
 * - `reconstruct_locations(ast, source)` — one-shot: build the line table once,
 *   walk the tree, add `loc` to every node, return the (mutated) ast.
 * - `create_locator(source)` — amortized: hold the prebuilt line table and expose
 *   `loc_of(node)` (single node) and `reconstruct(ast)` (whole tree). Prefer this
 *   for heavy sparse use; the bare `loc_of(node, source)` convenience rebuilds the
 *   O(source) table per call.
 *
 * Offsets are UTF-16 and JS strings are UTF-16-indexed, so a line-start table
 * scanned off the source string is directly comparable — no byte handling.
 * Column is 0-based UTF-16 code units; line is 1-based.
 *
 * **TypeScript is exact.** For a `.ts`/`.js` (`Program`) tree, each node's
 * reconstructed `loc` *value* equals the acorn `loc` the loc-bearing wire would
 * have emitted, exactly. (The key is *appended last* on each node rather than
 * placed after `start`/`end`, so an object consumer — or `deepEqual` — sees
 * identical data, but a re-serialized tree won't byte-match the wire's key order.)
 *
 * **Svelte is approximate** — reconstruct where you have the source, but be aware
 * of three deliberate divergences from Svelte's own wire (this module does NOT
 * replicate Svelte's parser quirks):
 * - `name_loc` is not recovered — it's dropped entirely from the span-only Svelte
 *   wire, and an element/attribute/directive name span is a Svelte-specific field
 *   with no offset on the node to derive it from.
 * - The `<script>`/`<style>` `Program` `loc` is Svelte's *tag-position* override
 *   (`read_script`), not the content offset the node's `start`/`end` carry, so the
 *   reconstructed `Program` `loc` is the content position, not Svelte's.
 * - Destructure patterns in `{#each … as …}` / `{:then}` / `{:catch}` / `{@const}`
 *   carry a `+1` column in Svelte's wire (parsed under a synthetic `(`); the
 *   reconstruction is the true offset, so it reads one column earlier.
 * - Additionally, Svelte's own wire carries `loc` *only* on embedded ECMAScript
 *   nodes (script + template expressions); this walk adds `loc` to the template
 *   nodes (elements, text, blocks) too, so the result is a superset. Everything
 *   outside the three cases above reconstructs exactly.
 *
 * **CSS is a no-op** — `parse_css` emits no `loc` (nothing to reconstruct), so
 * `reconstruct_locations` returns a CSS tree unchanged.
 *
 * The one-shot and `reconstruct` forms **mutate the ast in place** (adding a `loc`
 * key to each node) and return it, for efficiency on large trees. Callers that
 * need the input untouched should `structuredClone(ast)` first.
 *
 * @module
 */

const LF = 0x0a;
const CR = 0x0d;
const LINE_SEPARATOR = 0x2028;
const PARAGRAPH_SEPARATOR = 0x2029;

/**
 * The line-terminator rule for a language. TypeScript/JS follow ECMAScript
 * LineTerminators (`\n`, `\r`, `\r\n` as one, U+2028, U+2029); Svelte uses
 * LF-only across the whole document (incl. embedded `<script>`/`{expr}`),
 * matching the Svelte parser's locate-character convention.
 * @param {string} language
 * @returns {'ecmascript' | 'lf'}
 */
function rule_for(language) {
	return language === 'svelte' ? 'lf' : 'ecmascript';
}

/**
 * Line-start offsets (UTF-16 units); the rightmost start `<=` an offset gives its
 * line. Built once per source and reused for every `loc_at` lookup.
 * @param {string} source
 * @param {'ecmascript' | 'lf'} rule
 * @returns {number[]}
 */
function build_line_starts(source, rule) {
	const starts = [0];
	const ecmascript = rule === 'ecmascript';
	for (let i = 0; i < source.length; i++) {
		const c = source.charCodeAt(i);
		if (c === LF) {
			starts.push(i + 1);
		} else if (ecmascript) {
			if (c === CR) {
				if (source.charCodeAt(i + 1) === LF) i++; // \r\n counts as one line break
				starts.push(i + 1);
			} else if (c === LINE_SEPARATOR || c === PARAGRAPH_SEPARATOR) {
				starts.push(i + 1);
			}
		}
	}
	return starts;
}

/**
 * Line/column for a UTF-16 offset, via binary search over the line-start table.
 * @param {number} offset
 * @param {number[]} starts
 * @returns {{line: number, column: number}}
 */
function loc_at(offset, starts) {
	let lo = 0;
	let hi = starts.length - 1;
	while (lo < hi) {
		const mid = (lo + hi + 1) >> 1;
		if (starts[mid] <= offset) lo = mid;
		else hi = mid - 1;
	}
	return { line: lo + 1, column: offset - starts[lo] };
}

/**
 * Guess the language from the AST root when `opts.language` is omitted:
 * `Root` → svelte, `Program` → typescript, `StyleSheetFile` → css. Defaults to
 * typescript for anything else.
 * @param {any} ast
 * @returns {'typescript' | 'svelte' | 'css'}
 */
function infer_language(ast) {
	if (ast && typeof ast === 'object') {
		switch (ast.type) {
			case 'Root':
				return 'svelte';
			case 'Program':
				return 'typescript';
			case 'StyleSheetFile':
				return 'css';
		}
	}
	return 'typescript';
}

/**
 * Walk `value`, adding a `loc` object to every node with numeric `start`/`end`.
 * Mutates in place. Skips the `loc` key it writes so it never re-walks its own
 * output.
 * @param {any} value
 * @param {number[]} starts
 */
function walk_add_loc(value, starts) {
	if (Array.isArray(value)) {
		for (const v of value) walk_add_loc(v, starts);
	} else if (value && typeof value === 'object') {
		if (typeof value.start === 'number' && typeof value.end === 'number') {
			value.loc = { start: loc_at(value.start, starts), end: loc_at(value.end, starts) };
		}
		for (const key of Object.keys(value)) {
			if (key === 'loc') continue;
			walk_add_loc(value[key], starts);
		}
	}
}

/**
 * Build a locator that holds the source's line-start table so repeated lookups
 * don't rebuild it. Prefer this over the bare `loc_of`/`reconstruct_locations`
 * helpers for heavy sparse use — those rebuild the O(source) table per call.
 *
 * @param {string} source - the exact source the span-only wire was parsed from.
 * @param {{language?: 'typescript' | 'svelte' | 'css'}} [opts] - line rule
 *   selector; defaults to `typescript` (ECMAScript line terminators). Pass
 *   `svelte` for a `.svelte` document (LF-only), `css` for a no-op reconstruct.
 * @returns {{loc_of: (node: any) => ({start: {line: number, column: number}, end: {line: number, column: number}} | null), reconstruct: (ast: any) => any}}
 */
export function create_locator(source, opts) {
	const language = opts?.language ?? 'typescript';
	const starts = build_line_starts(source, rule_for(language));
	const is_css = language === 'css';
	return {
		loc_of(node) {
			if (!node || typeof node.start !== 'number' || typeof node.end !== 'number') {
				return null;
			}
			return { start: loc_at(node.start, starts), end: loc_at(node.end, starts) };
		},
		reconstruct(ast) {
			// CSS has no `loc` in the wire — nothing to reconstruct.
			if (!is_css) walk_add_loc(ast, starts);
			return ast;
		},
	};
}

/**
 * Add a `loc: {start, end}` line/column object to every node of a span-only wire,
 * derived from each node's `start`/`end` offsets + `source`. Builds the line-start
 * table once, **mutates `ast` in place**, and returns it. `structuredClone(ast)`
 * first if you need the input untouched.
 *
 * Exact for TypeScript; approximate for Svelte; a no-op for CSS — see the module
 * doc for the specifics.
 *
 * @param {any} ast - the span-only AST from `parse_*_no_locations` (untyped: the
 *   no-locations wire has no `.d.ts`).
 * @param {string} source - the exact source `ast` was parsed from.
 * @param {{language?: 'typescript' | 'svelte' | 'css'}} [opts] - line rule
 *   selector; inferred from the root node (`Root`/`Program`/`StyleSheetFile`) when
 *   omitted.
 * @returns {any} the same `ast`, now with `loc` on every node.
 */
export function reconstruct_locations(ast, source, opts) {
	const language = opts?.language ?? infer_language(ast);
	return create_locator(source, { language }).reconstruct(ast);
}

/**
 * Line/column for a single node, derived from its `start`/`end` + `source`.
 * Returns `null` if the node has no numeric `start`/`end`.
 *
 * Convenience form: it rebuilds the O(source) line-start table on every call, so
 * for more than a couple of lookups against one source reuse a `create_locator`.
 *
 * @param {any} node - a node from a span-only wire (must carry numeric `start`/`end`).
 * @param {string} source - the exact source the node was parsed from.
 * @param {{language?: 'typescript' | 'svelte' | 'css'}} [opts] - line rule
 *   selector; defaults to `typescript`. Pass `svelte` for a `.svelte` node.
 * @returns {{start: {line: number, column: number}, end: {line: number, column: number}} | null}
 */
export function loc_of(node, source, opts) {
	const language = opts?.language ?? 'typescript';
	return create_locator(source, { language }).loc_of(node);
}

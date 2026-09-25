/**
 * Line/column reconstruction for tsv's `no-locations` parse wire.
 *
 * A `parse_*` call with `{locations: false}` emits a span-only AST: every node keeps its
 * `start`/`end` (UTF-16 code-unit offsets) but drops the per-node `loc`
 * (line/column) object (Svelte also drops the element/attribute/directive
 * `name_loc`). Line/column is a pure function of an offset plus the source, so a
 * consumer holding only the span-only wire recovers both on demand — no re-parse.
 * This module is that reconstruction, shipped so callers don't reimplement the
 * line rules.
 *
 * Two entry points:
 * - `reconstruct_locations(ast, source)` — one-shot: build the line table once,
 *   walk the tree, add `loc` to every node (and `name_loc` back to the Svelte
 *   nodes that carry one), return the (mutated) ast.
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
 * **Svelte needs one line class, and refuses when the source has two.** A Svelte
 * document's `loc`s are LF-only (Svelte's `locate-character`) *except* on the nodes
 * acorn parsed, which carry acorn's ECMAScript line count — seeded once per embedded
 * parse, over whatever prefix Svelte prepared for it. The two classes agree unless the
 * source holds a lone `\r`, U+2028 or U+2029 (a `\r\n` is one ECMAScript break holding
 * one LF, so it never counts), and where they disagree the span-only wire does not carry
 * what would be needed to tell the classes apart: which acorn parse a node came from is
 * not a function of its offsets. So `create_locator` — and every entry point through it —
 * **throws** on such a source rather than returning lines that are quietly wrong for
 * every acorn-owned node. Parse with `loc` (the default) for those documents. Everything
 * else reconstructs as described below.
 *
 * **A block binding's type annotation is placed by the tree, not the offsets.** Svelte reads
 * a block binding's `: T` (`{#each xs as e: T}`, `{:then v: T}`, `{:catch e: T}`,
 * `{@const a: T = …}`) with its own acorn parse over a rewritten template: the five code
 * units ending at the colon are overwritten with a synthetic `_ as `. A newline in the four
 * units before the colon is therefore never counted, and the annotation's nodes sit that many
 * lines higher (`{#each xs as⏎e: T}` puts `T` on the `as` line); a destructure binding's
 * `loc.end` also stops at its closing bracket though its `end` runs over the annotation.
 * Both are exact functions of the binding, so the reconstruction applies them — but they need
 * the *tree* to find the binding, which is why a Svelte `loc_of` takes the span-only `ast`
 * (see `create_locator`) and throws without it, rather than answer one node differently from
 * `reconstruct`.
 *
 * **Why refuse rather than carry what is missing.** The span-only wire could ship the
 * per-parse origins that make the two-line-class case derivable — a few dozen bytes per
 * document against a `loc` on every node. It deliberately does not. Reconstructing them here
 * would mean a second implementation, in JS, of a model that exists only to mirror upstream
 * defects: acorn seeds `lineStart` with `lastIndexOf("\n", …)` while counting `curLine` over
 * the full terminator class, and Svelte prepares acorn's input three different ways across
 * five readers. That model moves when either upstream moves, nothing in `deno task check`
 * would grade the JS copy against the Rust one, and the documents it buys back hold a lone CR,
 * U+2028 or U+2029. Parsing those with `loc` is the better answer.
 *
 * **Svelte is otherwise approximate** — reconstruct where you have the source, but be
 * aware of two deliberate divergences from Svelte's own wire (this module does NOT
 * replicate these two parser quirks):
 * - The `<script>`/`<style>` `Program` `loc` is Svelte's *tag-position* override
 *   (`read_script`), not the content offset the node's `start`/`end` carry, so the
 *   reconstructed `Program` `loc` is the content position, not Svelte's.
 * - Destructure patterns in `{#each … as …}` / `{:then}` / `{:catch}` / `{@const}`
 *   carry a `+1` column in Svelte's wire (parsed under a synthetic `(`) on every endpoint
 *   that sits on the line the pattern opens on, unless that is line 1 — the pattern's own
 *   `loc.end` included, when it closes there; the reconstruction is the true offset, so it
 *   reads one column earlier.
 *
 * Additionally, Svelte's own wire carries `loc` *only* on embedded ECMAScript
 * nodes (script + template expressions); this walk adds `loc` to the template
 * nodes (elements, text, blocks) too, so the result is a superset. Everything
 * outside the two cases above reconstructs exactly.
 *
 * **Svelte `name_loc` is exact.** The name span is a function of the node's own
 * `start`/`end` + type — a tag name is the run after `<`, an attribute name starts
 * at the node (a shorthand `{x}` names the identifier inside the braces, so `{ x }`
 * excludes the padding), and a directive names its whole head token
 * (`on:click|preventDefault`) — so the walk
 * restores `name_loc` (`{line, column, character}` endpoints) on every element,
 * attribute, and directive that carries one, matching Svelte's wire.
 *
 * That same name shape reaches a few identifiers: Svelte reports `{line, column,
 * character}` on the ones its own reader creates — a shorthand attribute's
 * expansion (`{x}`), a snippet name, and a simple-identifier block pattern
 * (`{#each … as x}`, `{:then x}`, `{:catch x}`, `{@const x = …}`) — so the walk
 * gives those the `character` field too. The last `character`-bearing shape is the
 * **in-tag comment** — one written between an element's attributes, which Svelte's
 * template reader collects rather than acorn. Nothing on the comment node marks it,
 * so it's recovered structurally (see `stamp_in_tag_comment_locs`) and gets the
 * `character` field like Svelte's.
 *
 * **CSS is a no-op** — `parse_css` emits no `loc` (nothing to reconstruct), so
 * `reconstruct_locations` returns a CSS tree unchanged.
 *
 * **A leading byte-order mark is read the way each wire reads it.** Svelte's `parse` and
 * `parseCss` strip a U+FEFF at index 0 before parsing, so the Svelte and CSS wires index
 * the BOM-less string; acorn counts it as whitespace, so the TypeScript wire indexes the
 * caller's string as given. The line table and every name span are built over the string
 * the wire's offsets index — `source` with its BOM dropped for Svelte and CSS, `source`
 * itself for TypeScript — so a consumer hands over the source it parsed, BOM and all.
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
const BOM = 0xfeff;

/**
 * The string a language's wire offsets index: `source` itself for TypeScript (acorn counts
 * a leading BOM as whitespace), `source` without a leading U+FEFF for Svelte and CSS
 * (Svelte's `parse` and `parseCss` strip it before parsing — `remove_bom` — so their
 * offsets are one unit lower than the file's from the first character on).
 * @param {string} source
 * @param {'typescript' | 'svelte' | 'css'} language
 * @returns {string}
 */
function indexed_text(source, language) {
	if (language !== 'typescript' && source.charCodeAt(0) === BOM) return source.slice(1);
	return source;
}

/**
 * The line-terminator rule for a language. TypeScript/JS follow ECMAScript
 * LineTerminators (`\n`, `\r`, `\r\n` as one, U+2028, U+2029); Svelte uses
 * LF-only, matching the Svelte parser's locate-character convention.
 *
 * One rule per document is only enough for Svelte because `create_locator` has
 * already refused any source whose acorn-owned nodes would need the other one —
 * see `ECMASCRIPT_ONLY_TERMINATOR` and the module doc — and places a block binding's
 * annotation itself (`push_binding_annotations`).
 * @param {string} language
 * @returns {'ecmascript' | 'lf'}
 */
function rule_for(language) {
	return language === 'svelte' ? 'lf' : 'ecmascript';
}

/**
 * A terminator the ECMAScript class counts and the LF class does not — a lone `\r`,
 * U+2028, or U+2029. `\r\n` is one ECMAScript break holding one LF, so the two classes
 * agree over it and the negative lookahead is what keeps it out.
 *
 * The JS twin of the Rust probe `LocationTracker::new_with_map` returns (which is what
 * decides, on the emitting side, whether a second line table is built at all), and it must
 * keep answering the same question: a source this says `false` about is one where every
 * acorn seed is the identity and a single LF table reproduces the whole Svelte wire.
 */
const ECMASCRIPT_ONLY_TERMINATOR = /\r(?!\n)|[\u2028\u2029]/;

/**
 * Refuse a reconstruction, naming why.
 *
 * The refusal states one fact — the span-only wire records a node's offsets but not which
 * acorn parse produced them — behind a stable marker, `cannot reconstruct \`loc\``, a caller
 * (or a test) can match on; the varying half is the cause, and the remedy is always the same.
 *
 * @param {string} cause - what the source or tree holds, as a clause.
 * @returns {never}
 */
function refuse(cause) {
	throw new Error(
		'tsv: cannot reconstruct `loc` for this Svelte document because ' +
			cause +
			', and the span-only wire does not record which acorn parse a node came from. ' +
			'Parse this document with locations (the default) instead.'
	);
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
 * Index into `starts` of the line containing `offset` — the rightmost start `<=`
 * it, by binary search. The two point shapes (`loc`'s `{line, column}` and
 * `name_loc`'s `{line, column, character}`) build on it directly, so neither has
 * to construct the other's object and discard it.
 * @param {number} offset
 * @param {number[]} starts
 * @returns {number}
 */
function line_index_at(offset, starts) {
	let lo = 0;
	let hi = starts.length - 1;
	while (lo < hi) {
		const mid = (lo + hi + 1) >> 1;
		if (starts[mid] <= offset) lo = mid;
		else hi = mid - 1;
	}
	return lo;
}

/**
 * Line/column for a UTF-16 offset.
 * @param {number} offset
 * @param {number[]} starts
 * @returns {{line: number, column: number}}
 */
function loc_at(offset, starts) {
	const i = line_index_at(offset, starts);
	return { line: i + 1, column: offset - starts[i] };
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
 * Where a Svelte node's `name_loc` span sits, by node type — one lookup for the
 * walk's hottest question (most nodes carry no `name_loc`, and `Identifier` has a
 * `name` field without one):
 * - `element` — the tag-name run right after the opening `<`.
 * - `directive` — the whole head token (`on:click|preventDefault`: prefix, name,
 *   and modifiers), from the node start to the first name terminator. `in:`/`out:`
 *   are `TransitionDirective`, so they need no entry of their own.
 * - `attribute` — the name run at the node start, or a shorthand's identifier
 *   inside the braces.
 * @type {Map<string, 'element' | 'directive' | 'attribute'>}
 */
const NAME_LOC_KINDS = new Map([
	['RegularElement', 'element'],
	['Component', 'element'],
	['SvelteHead', 'element'],
	['SvelteWindow', 'element'],
	['SvelteBody', 'element'],
	['SvelteDocument', 'element'],
	['SvelteElement', 'element'],
	['SvelteComponent', 'element'],
	['SvelteSelf', 'element'],
	['SlotElement', 'element'],
	['SvelteFragment', 'element'],
	['SvelteBoundary', 'element'],
	['TitleElement', 'element'],
	['OnDirective', 'directive'],
	['BindDirective', 'directive'],
	['ClassDirective', 'directive'],
	['StyleDirective', 'directive'],
	['UseDirective', 'directive'],
	['TransitionDirective', 'directive'],
	['AnimateDirective', 'directive'],
	['LetDirective', 'directive'],
	['Attribute', 'attribute']
]);

/**
 * The chars that end an attribute/directive name run — Svelte's
 * `regex_token_ending_character` verbatim, which is also what tsv's parser
 * implements, so the derived span always agrees with the wire it reconstructs.
 *
 * Spelled as the regex rather than a character list on purpose: `\s` is Unicode
 * (U+00A0, U+3000, U+FEFF, …), so an enumerated ASCII list would silently under-match
 * and swallow a Unicode space into a name. Sharing the literal makes drift impossible.
 */
const NAME_TERMINATORS = /[\s=/>"']/;

/**
 * Whether an `Attribute` is the shorthand form (`{x}`, or padded: `{ x }`), whose
 * name sits inside the braces rather than at the node start. A `<script>`/`<style>`
 * attribute name is a literal run that can itself be braced (`<script {x}>`, name
 * `{x}`), so a verbatim name at the node start is not one.
 * @param {any} node
 * @param {string} source
 * @returns {boolean}
 */
function is_shorthand_attribute(node, source) {
	if (typeof node.name !== 'string' || typeof node.start !== 'number') return false;
	return source[node.start] === '{' && !source.startsWith(node.name, node.start);
}

/**
 * The `[start, end]` UTF-16 offsets a Svelte node's `name_loc` covers, or `null`
 * for a node type that carries none.
 * @param {any} node
 * @param {string} source
 * @returns {[number, number] | null}
 */
function name_span_of(node, source) {
	const kind = NAME_LOC_KINDS.get(node.type);
	if (kind === undefined || typeof node.name !== 'string') return null;
	if (kind === 'element') {
		const start = node.start + 1; // the tag name follows `<`
		return [start, start + node.name.length];
	}
	if (kind === 'directive') {
		let end = node.start;
		while (end < node.end && !NAME_TERMINATORS.test(source[end])) end++;
		return [node.start, end];
	}
	if (is_shorthand_attribute(node, source)) {
		// `{ x }` names just the `x` — Svelte's reader ate the padding before reading the
		// identifier. The padding is whitespace, so the first occurrence of the name at or
		// after the `{` IS the identifier, whichever whitespace chars the padding used.
		const name_start = source.indexOf(node.name, node.start + 1);
		if (name_start < 0 || name_start >= node.end) return null;
		return [name_start, name_start + node.name.length];
	}
	return [node.start, node.start + node.name.length];
}

/**
 * The `Identifier` a shorthand attribute expands to (`{x}` → `x`), or `null`.
 * @param {any} node - a shorthand `Attribute`.
 * @returns {any | null}
 */
function shorthand_identifier_of(node) {
	// a shorthand's `value` is the bare `ExpressionTag`; a quoted value is an array
	const tag = Array.isArray(node.value)
		? node.value.find((v) => v?.type === 'ExpressionTag')
		: node.value;
	if (tag?.type !== 'ExpressionTag') return null;
	return tag.expression?.type === 'Identifier' ? tag.expression : null;
}

/**
 * A `name_loc` endpoint — line/column plus the offset itself, the extra
 * `character` field Svelte's `name_loc` carries and `loc` does not.
 * @param {number} offset
 * @param {number[]} starts
 * @returns {{line: number, column: number, character: number}}
 */
function name_loc_at(offset, starts) {
	const i = line_index_at(offset, starts);
	return { line: i + 1, column: offset - starts[i], character: offset };
}

/**
 * Overwrite `node`'s plain `loc` with the name-shaped one, if it's an identifier.
 * @param {any} node
 * @param {number[]} starts
 * @mutates node
 */
function stamp_name_shaped_loc(node, starts) {
	if (node?.type !== 'Identifier') return;
	if (typeof node.start !== 'number' || typeof node.end !== 'number') return;
	node.loc = { start: name_loc_at(node.start, starts), end: name_loc_at(node.end, starts) };
}

/**
 * Give the identifiers under `node` the name-shaped `loc` — the `{line, column,
 * character}` endpoints Svelte reports on the ones its own reader creates, rather
 * than the plain `{line, column}` an acorn-parsed node carries. Those are a
 * shorthand attribute's expansion (`{x}`), a snippet name, and a block pattern
 * that is a simple identifier (`{#each … as x}`, `{:then x}`, `{:catch x}`,
 * `{@const x = …}` — a destructure pattern takes the `+1`-column quirk instead).
 * @param {any} node
 * @param {number[]} starts
 * @param {string} source
 * @mutates node
 */
function stamp_character_locs(node, starts, source) {
	switch (node.type) {
		case 'Attribute':
			if (is_shorthand_attribute(node, source)) {
				stamp_name_shaped_loc(shorthand_identifier_of(node), starts);
			}
			return;
		case 'SnippetBlock':
			stamp_name_shaped_loc(node.expression, starts);
			return;
		case 'EachBlock':
			stamp_name_shaped_loc(node.context, starts);
			return;
		case 'AwaitBlock':
			stamp_name_shaped_loc(node.value, starts);
			stamp_name_shaped_loc(node.error, starts);
			return;
		case 'ConstTag':
			for (const d of node.declaration?.declarations ?? []) stamp_name_shaped_loc(d?.id, starts);
	}
}

/**
 * Whether `comment` sits *between* `element`'s attributes — the position Svelte's
 * own template reader collects from.
 *
 * Scans `element`'s opening tag tracking brace depth and quoting, and reports
 * whether the comment begins at depth 0 outside any quoted value. Everything
 * brace-wrapped is an expression acorn parses (an attribute value, a directive, a
 * spread, an `{@attach}`, a `svelte:element` `this={…}` binding), so one depth test
 * covers them all — no field-name knowledge, and no reliance on a `tag`/`expression`
 * span the wire may not carry. Comment bytes are stepped over whole, so a `>`, `{`,
 * or quote written inside a comment is never read as structure.
 * @param {any} element
 * @param {{start: number}} comment
 * @param {string} source
 * @param {Map<number, any>} comments_by_start - the root comment list, keyed by
 * `start` — one lookup per scanned character, where a `find` over the list made
 * the scan O(characters × comments) per comment.
 * @returns {boolean}
 */
function is_between_attributes(element, comment, source, comments_by_start) {
	let i = element.start + 1; // past `<`
	let depth = 0;
	let quote = '';
	while (i < source.length) {
		if (quote === '') {
			const here = comments_by_start.get(i);
			if (here) {
				if (i === comment.start) return depth === 0;
				i = here.end;
				continue;
			}
		}
		const ch = source[i];
		if (quote !== '') {
			if (ch === quote) quote = '';
		} else if (ch === '"' || ch === "'") {
			quote = ch;
		} else if (ch === '{') {
			depth++;
		} else if (ch === '}') {
			if (depth > 0) depth--;
		} else if (ch === '>' && depth === 0) {
			return false; // opening tag closed before reaching the comment
		}
		i++;
	}
	return false;
}

/**
 * Give each **in-tag** comment the `character`-bearing `loc` Svelte reports on it.
 *
 * Svelte's template reader collects the comments written *between* an element's
 * attributes (`<div /* c *\/ class="x">`) and stamps `character` into their `loc`;
 * every other comment is collected by acorn and gets the plain shape. Nothing on
 * the comment node itself tells the two apart, so the class is recovered
 * structurally — a comment is in-tag when it sits between the attributes of the
 * innermost element containing it. The "between" half is load-bearing: a comment
 * inside an attribute's *expression* (`{@attach /* c *\/ foo}`, `onclick={() =>
 * /* c *\/ x}`, `<svelte:element this={/* c *\/ 'p'}>`) is inside the opening tag
 * too, but acorn parses it, so it keeps the plain shape.
 * @param {any[]} comments
 * @param {any[]} elements - every element node the walk passed, in visit order.
 * @param {number[]} starts
 * @param {string} source
 * @mutates comments
 */
function stamp_in_tag_comment_locs(comments, elements, starts, source) {
	const comments_by_start = new Map();
	for (const c of comments) {
		if (typeof c?.start === 'number') comments_by_start.set(c.start, c);
	}
	for (const c of comments) {
		if (typeof c?.start !== 'number' || typeof c.end !== 'number') continue;
		// innermost = the containing element with the greatest start; an outer
		// element's opening tag closes before the comment, so only this one can hold it
		let host = null;
		for (const e of elements) {
			if (e.start < c.start && c.end <= e.end && (host === null || e.start > host.start)) {
				host = e;
			}
		}
		if (host === null || !is_between_attributes(host, c, source, comments_by_start)) continue;
		c.loc = { start: name_loc_at(c.start, starts), end: name_loc_at(c.end, starts) };
	}
}

/**
 * Svelte's template whitespace class — what `allow_whitespace` skips between a block
 * binding and its `:` — which is exactly JavaScript's `\s`.
 */
const TEMPLATE_WHITESPACE = /\s/;

/**
 * @typedef {object} BindingAnnotation
 * @property {any} pattern - the binding pattern the annotation hangs off.
 * @property {number} close - the destructure pattern's real end (its annotation's
 *   `start`), or `-1` for an identifier binding, whose `end` was never stretched.
 * @property {number} colon - the annotation's `:`.
 * @property {number} annotation_end
 * @property {number} erased - newlines the synthetic `_ as ` overwrote.
 * @property {number} colon_line - line index of the colon.
 * @property {number} base_line - line index where the rewrite began (four units before the colon).
 * @property {number} base - that line's start offset: the column origin acorn kept.
 */

/**
 * The positions a Svelte **block binding's** type annotation shifts, one entry per
 * annotated binding that needs one.
 *
 * A block binding (`{#each … as P: T}`, `{:then P: T}`, `{:catch P: T}`,
 * `{@const P: T = …}`) is read by Svelte's own `read_pattern`, and its `: T` by
 * `read_type_annotation`, which runs a **second** acorn parse over a rewritten template:
 * the five code units ending at the colon become a synthetic `_ as `, and everything ahead
 * of them is blanked with the newlines kept. Two facts of the wire follow, and neither is a
 * function of a node's own offsets:
 *
 * - **Erased newlines.** A newline in the four code units before the colon is overwritten,
 *   so acorn never counts it: every node inside the annotation sits that many lines higher,
 *   and one on the colon's own line takes its column from the line the rewrite began on
 *   (`{#each xs as⏎e: T}` puts `T` on the `as` line). A newline further back survives the
 *   blanking, so `{#each xs as e⏎        : T}` reads plainly.
 * - **The pattern's own end.** A destructure pattern's `end` is stretched over the
 *   annotation after acorn has placed it, so its `loc.end` still names the closing bracket
 *   — the annotation's `start`.
 *
 * These annotations are the only ones Svelte builds by hand (they carry no `loc` of their
 * own), and they are found by the slot they sit in, never by their shape: an acorn-built
 * annotation (a parameter's, a variable's) can look identical in the span-only wire.
 *
 * @param {any} node - a Svelte node; only `EachBlock`, `AwaitBlock` and `ConstTag` yield entries.
 * @param {number[]} starts
 * @param {string} source
 * @param {BindingAnnotation[]} out
 * @mutates out - one entry appended per annotated binding that needs one.
 */
function push_binding_annotations(node, starts, source, out) {
	switch (node.type) {
		case 'EachBlock':
			push_binding_annotation(node.context, starts, source, out);
			return;
		case 'AwaitBlock':
			push_binding_annotation(node.value, starts, source, out);
			push_binding_annotation(node.error, starts, source, out);
			return;
		case 'ConstTag':
			for (const d of node.declaration?.declarations ?? []) {
				push_binding_annotation(d?.id, starts, source, out);
			}
	}
}

/**
 * The entry for one block binding, if its annotation needs one — see
 * `push_binding_annotations`.
 * @param {any} pattern - the binding pattern (an `Identifier`, `ObjectPattern` or `ArrayPattern`).
 * @param {number[]} starts
 * @param {string} source
 * @param {BindingAnnotation[]} out
 * @mutates out
 */
function push_binding_annotation(pattern, starts, source, out) {
	const annotation = pattern?.typeAnnotation;
	if (annotation?.type !== 'TSTypeAnnotation' || typeof annotation.start !== 'number') return;
	// the colon is where Svelte's `allow_whitespace` stops, as its reader found it
	let colon = annotation.start;
	while (colon < source.length && TEMPLATE_WHITESPACE.test(source[colon])) colon++;
	if (source[colon] !== ':') return;
	const window_start = Math.max(0, colon - 4);
	let erased = 0;
	for (let i = window_start; i < colon; i++) if (source.charCodeAt(i) === LF) erased++;
	const destructure = pattern.type === 'ObjectPattern' || pattern.type === 'ArrayPattern';
	if (erased === 0 && !destructure) return;
	const base_line = line_index_at(window_start, starts);
	out.push({
		pattern,
		close: destructure ? annotation.start : -1,
		colon,
		annotation_end: annotation.end,
		erased,
		colon_line: line_index_at(colon, starts),
		base_line,
		base: starts[base_line]
	});
}

/**
 * One offset inside a block binding's annotation, as the annotation's own acorn parse
 * counted it: `erased` lines higher, and on the colon's line measured from where the
 * rewrite began.
 * @param {number} offset
 * @param {BindingAnnotation} b
 * @param {number[]} starts
 * @returns {{line: number, column: number}}
 */
function annotation_loc_at(offset, b, starts) {
	const i = line_index_at(offset, starts);
	if (i === b.colon_line) return { line: b.base_line + 1, column: offset - b.base };
	return { line: i + 1 - b.erased, column: offset - starts[i] };
}

/**
 * A node's `loc`: the plain line table's answer, unless a block binding in `bindings`
 * places it — a node inside an annotation whose newlines were erased, or the destructure
 * pattern whose `end` the annotation stretched. `bindings` is empty for every
 * non-Svelte tree and for nearly every Svelte one, which is the fast path.
 * @param {any} node - a node with numeric `start`/`end`.
 * @param {number[]} starts
 * @param {BindingAnnotation[]} bindings
 * @returns {{start: {line: number, column: number}, end: {line: number, column: number}}}
 */
function node_loc(node, starts, bindings) {
	for (let k = 0; k < bindings.length; k++) {
		const b = bindings[k];
		if (b.erased > 0 && node.start > b.colon && node.end <= b.annotation_end) {
			return {
				start: annotation_loc_at(node.start, b, starts),
				end: annotation_loc_at(node.end, b, starts)
			};
		}
		if (
			b.close >= 0 &&
			node.start === b.pattern.start &&
			node.end === b.pattern.end &&
			node.type === b.pattern.type
		) {
			return { start: loc_at(node.start, starts), end: loc_at(b.close, starts) };
		}
	}
	return { start: loc_at(node.start, starts), end: loc_at(node.end, starts) };
}

/**
 * Walk `value`, adding a `loc` object to every node with numeric `start`/`end` —
 * and, for a Svelte tree, a `name_loc` to every element, attribute, and directive
 * that carries one. Mutates in place. Skips the keys it writes so it never
 * re-walks its own output.
 * @param {any} value
 * @param {{starts: number[], source: string, is_svelte: boolean, elements: any[] | null, bindings: BindingAnnotation[], collected: BindingAnnotation[]}} ctx
 *   `elements` collects element nodes for the in-tag comment pass, or is `null`
 *   when the document has no comments to classify. `bindings` holds the block-binding
 *   annotations of the blocks the walk is inside — pushed on the way in, dropped on the
 *   way out, so it never grows past the nesting depth — and `collected` every one it has
 *   met, for the root `comments` list (`reconstruct_in`).
 */
function walk_add_loc(value, ctx) {
	if (Array.isArray(value)) {
		for (const v of value) walk_add_loc(v, ctx);
	} else if (value && typeof value === 'object') {
		const depth = ctx.bindings.length;
		if (ctx.is_svelte) {
			push_binding_annotations(value, ctx.starts, ctx.source, ctx.bindings);
			// kept for the root `comments` list, which the walk reaches outside every block
			for (let i = depth; i < ctx.bindings.length; i++) ctx.collected.push(ctx.bindings[i]);
		}
		if (typeof value.start === 'number' && typeof value.end === 'number') {
			// the empty-`bindings` fast path: every TypeScript tree, and nearly every Svelte one
			value.loc =
				ctx.bindings.length === 0
					? { start: loc_at(value.start, ctx.starts), end: loc_at(value.end, ctx.starts) }
					: node_loc(value, ctx.starts, ctx.bindings);
			if (ctx.is_svelte) {
				const span = name_span_of(value, ctx.source);
				if (span) {
					value.name_loc = {
						start: name_loc_at(span[0], ctx.starts),
						end: name_loc_at(span[1], ctx.starts)
					};
				}
				if (ctx.elements !== null && NAME_LOC_KINDS.get(value.type) === 'element') {
					ctx.elements.push(value);
				}
			}
		}
		for (const key of Object.keys(value)) {
			if (key === 'loc' || key === 'name_loc') continue;
			walk_add_loc(value[key], ctx);
		}
		if (ctx.bindings.length !== depth) ctx.bindings.length = depth;
		// Re-stamp the identifiers Svelte gives the name-shaped `loc`, after the walk
		// above wrote them the plain shape.
		if (ctx.is_svelte) stamp_character_locs(value, ctx.starts, ctx.source);
	}
}

/**
 * The keys that lead from one Svelte **template** node to the next: a fragment's `nodes`,
 * an element's or `{#key}`'s `fragment`, and each block's branches. Block bindings live only
 * in the template, so the per-node `loc_of` collects them along these alone and never walks
 * the `<script>` programs, the stylesheet, or an expression subtree — which is most of a
 * component's tree. Every one of them holds a `Fragment` (or `null`) on every node that
 * carries it, so the walk cannot step off the template.
 */
const TEMPLATE_CHILD_KEYS = [
	'fragment',
	'nodes',
	'body',
	'fallback',
	'consequent',
	'alternate',
	'pending',
	'then',
	'catch'
];

/**
 * Every block-binding annotation entry in a Svelte tree, for the per-node `loc_of`, which
 * has no walk to collect them on the way down — gathered through the template alone
 * (`TEMPLATE_CHILD_KEYS`).
 * @param {any} value
 * @param {number[]} starts
 * @param {string} source
 * @param {BindingAnnotation[]} out
 * @returns {BindingAnnotation[]}
 */
function collect_binding_annotations(value, starts, source, out) {
	if (Array.isArray(value)) {
		for (const v of value) collect_binding_annotations(v, starts, source, out);
	} else if (value && typeof value === 'object') {
		push_binding_annotations(value, starts, source, out);
		for (const key of TEMPLATE_CHILD_KEYS) {
			if (value[key]) collect_binding_annotations(value[key], starts, source, out);
		}
	}
	return out;
}

/**
 * The whole reconstruction for one already-built line table: the `loc`/`name_loc`
 * walk plus the Svelte in-tag comment pass. Shared by both entry points so they
 * can't drift.
 * @param {any} ast
 * @param {number[]} starts
 * @param {string} source
 * @param {'typescript' | 'svelte' | 'css'} language
 * @returns {any} the same `ast`, mutated.
 */
function reconstruct_in(ast, starts, source, language) {
	// CSS has no `loc` in the wire — nothing to reconstruct.
	if (language === 'css') return ast;
	const is_svelte = language === 'svelte';
	const comments = is_svelte && Array.isArray(ast?.comments) ? ast.comments : null;
	const elements = comments !== null && comments.length > 0 ? [] : null;
	/** @type {BindingAnnotation[]} */
	const collected = [];
	walk_add_loc(ast, { starts, source, is_svelte, elements, bindings: [], collected });
	// A comment inside a block binding's annotation is listed twice: attached under the
	// annotation, where the walk placed it with its binding in scope, and in the root
	// `comments` list, which the walk reached outside every block. Re-place the root copies
	// against every binding the template holds — once, and only when there is one.
	if (comments !== null && collected.length > 0) {
		for (const c of comments) {
			if (typeof c?.start === 'number' && typeof c.end === 'number') {
				c.loc = node_loc(c, starts, collected);
			}
		}
	}
	if (elements !== null) {
		// `<svelte:options>` is the one attribute-bearing tag head whose wire node
		// carries no `type` (Svelte's `root.options`), so the type-keyed walk can't
		// collect it as a host — push it directly so its in-tag comments get the
		// `character`-bearing loc like any element's.
		if (ast?.options) elements.push(ast.options);
		stamp_in_tag_comment_locs(comments, elements, starts, source);
	}
	return ast;
}

/**
 * Build a locator that holds the source's line-start table so repeated lookups
 * don't rebuild it. Prefer this over the bare `loc_of`/`reconstruct_locations`
 * helpers for heavy sparse use — those rebuild the O(source) table per call.
 *
 * A Svelte locator's `loc_of` needs `opts.ast`, the span-only tree the nodes come from:
 * where a node inside a block binding's type annotation sits depends on that binding, which
 * the node alone does not say. The locator reads the tree's template once, up front (the
 * bindings live nowhere else — `TEMPLATE_CHILD_KEYS`); `loc_of` without it throws rather
 * than answer differently from `reconstruct`.
 *
 * @param {string} source - the exact source the span-only wire was parsed from.
 * @param {{language?: 'typescript' | 'svelte' | 'css', ast?: any}} [opts] - `language`
 *   selects the line rule: `typescript` (ECMAScript line terminators) by default, or
 *   inferred from `ast` when given; `svelte` for a `.svelte` document (LF-only), `css` for
 *   a no-op reconstruct. `ast` is the span-only tree, required by a Svelte `loc_of`.
 * @returns {{loc_of: (node: any) => ({start: {line: number, column: number}, end: {line: number, column: number}} | null), reconstruct: (ast: any) => any}}
 */
export function create_locator(source, opts) {
	const tree = opts?.ast;
	const language = opts?.language ?? (tree === undefined ? 'typescript' : infer_language(tree));
	// Everything below reads the string the wire's offsets index (see `indexed_text`),
	// never the caller's `source` directly — a Svelte BOM is not in the wire's coordinates.
	const text = indexed_text(source, language);
	if (language === 'svelte' && ECMASCRIPT_ONLY_TERMINATOR.test(text)) {
		refuse(
			'the source contains a lone CR, U+2028, or U+2029, so its acorn-parsed nodes carry ' +
				'a different line count from the rest of the document'
		);
	}
	const starts = build_line_starts(text, rule_for(language));
	const is_svelte = language === 'svelte';
	/** @type {BindingAnnotation[] | null} */
	const bindings = !is_svelte
		? []
		: tree === undefined
			? null
			: collect_binding_annotations(tree, starts, text, []);
	return {
		loc_of(node) {
			if (bindings === null) {
				throw new Error(
					'tsv: `loc_of` on a Svelte node needs the span-only tree it came from — pass ' +
						'`{ast}` to `create_locator`/`loc_of` — because a block binding places the ' +
						'nodes of its type annotation, and the node alone does not say which binding it is under.'
				);
			}
			if (!node || typeof node.start !== 'number' || typeof node.end !== 'number') {
				return null;
			}
			return node_loc(node, starts, bindings);
		},
		reconstruct(ast) {
			return reconstruct_in(ast, starts, text, language);
		}
	};
}

/**
 * Add a `loc: {start, end}` line/column object to every node of a span-only wire,
 * derived from each node's `start`/`end` offsets + `source` — and, for a Svelte
 * tree, the `name_loc` its elements, attributes, and directives carry. Builds the
 * line-start table once, **mutates `ast` in place**, and returns it.
 * `structuredClone(ast)` first if you need the input untouched.
 *
 * Exact for TypeScript; approximate for Svelte; a no-op for CSS — see the module
 * doc for the specifics.
 *
 * @param {any} ast - the span-only AST from a `{locations: false}` parse (untyped: the
 *   no-locations wire has no `.d.ts`).
 * @param {string} source - the exact source `ast` was parsed from.
 * @param {{language?: 'typescript' | 'svelte' | 'css'}} [opts] - line rule
 *   selector; inferred from the root node (`Root`/`Program`/`StyleSheetFile`) when
 *   omitted.
 * @returns {any} the same `ast`, now with `loc` on every node (plus `name_loc` on
 *   the Svelte nodes that carry one).
 */
export function reconstruct_locations(ast, source, opts) {
	const language = opts?.language ?? infer_language(ast);
	return create_locator(source, { language }).reconstruct(ast);
}

/**
 * Line/column for a single node, derived from its `start`/`end` + `source`.
 * Returns `null` if the node has no numeric `start`/`end`.
 *
 * Convenience form: it rebuilds the O(source) line-start table on every call (and, for
 * Svelte, re-walks the template of `opts.ast` — no larger than the source, and never cached,
 * since a tree the caller edits between calls would be served stale), so for more than a
 * couple of lookups against one source reuse a `create_locator`.
 *
 * @param {any} node - a node from a span-only wire (must carry numeric `start`/`end`).
 * @param {string} source - the exact source the node was parsed from.
 * @param {{language?: 'typescript' | 'svelte' | 'css', ast?: any}} [opts] - as for
 *   `create_locator`: `ast` is required for a Svelte node.
 * @returns {{start: {line: number, column: number}, end: {line: number, column: number}} | null}
 */
export function loc_of(node, source, opts) {
	return create_locator(source, opts).loc_of(node);
}

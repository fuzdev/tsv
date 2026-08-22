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
 * not a function of its offsets. So `create_locator` — and both entry points through it —
 * **throws** on such a source rather than returning lines that are quietly wrong for
 * every acorn-owned node. Parse with `loc` (the default) for those documents. Everything
 * else reconstructs as described below.
 *
 * **A second refusal, same cause one level down.** A block binding's `: T` is read by its
 * own acorn parse, entered five bytes behind the colon on a synthetic `_ as ` that
 * *overwrites* them — so a newline between the binding and its colon
 * (`{#each xs as e⏎: T}`) is erased before acorn sees it and the annotation's nodes stay on
 * the binding's line. That needs a per-parse seed, which the offsets cannot supply, so the
 * `reconstruct` forms throw on such a tree. It is checked against the AST rather than the
 * source (only a parse says where a block binding is), which is why this one fires from
 * `reconstruct` rather than from `create_locator`. Whitespace *without* a newline is fine:
 * the seed is then the identity.
 *
 * **Svelte is otherwise approximate** — reconstruct where you have the source, but be
 * aware of two deliberate divergences from Svelte's own wire (this module does NOT
 * replicate Svelte's parser quirks):
 * - The `<script>`/`<style>` `Program` `loc` is Svelte's *tag-position* override
 *   (`read_script`), not the content offset the node's `start`/`end` carry, so the
 *   reconstructed `Program` `loc` is the content position, not Svelte's.
 * - Destructure patterns in `{#each … as …}` / `{:then}` / `{:catch}` / `{@const}`
 *   carry a `+1` column in Svelte's wire (parsed under a synthetic `(`); the
 *   reconstruction is the true offset, so it reads one column earlier.
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
 * LF-only, matching the Svelte parser's locate-character convention.
 *
 * One rule per document is only enough for Svelte because `create_locator` has
 * already refused any source whose acorn-owned nodes would need the other one —
 * see `has_ecmascript_only_terminator` and the module doc.
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
 * Both refusals are the same fact in two spellings — the span-only wire records a node's
 * offsets but not which acorn parse produced them — so they share one message shape rather
 * than two that can drift. `cannot reconstruct \`loc\`` is the stable marker a caller (or a
 * test) can match on; the varying half is the cause, and the remedy is always the same.
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
 * @param {any[]} comments - the root comment list.
 * @returns {boolean}
 */
function is_between_attributes(element, comment, source, comments) {
	let i = element.start + 1; // past `<`
	let depth = 0;
	let quote = '';
	while (i < source.length) {
		if (quote === '') {
			const here = comments.find((c) => c.start === i);
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
		if (host === null || !is_between_attributes(host, c, source, comments)) continue;
		c.loc = { start: name_loc_at(c.start, starts), end: name_loc_at(c.end, starts) };
	}
}

/**
 * Walk `value`, adding a `loc` object to every node with numeric `start`/`end` —
 * and, for a Svelte tree, a `name_loc` to every element, attribute, and directive
 * that carries one. Mutates in place. Skips the keys it writes so it never
 * re-walks its own output.
 * @param {any} value
 * @param {{starts: number[], source: string, is_svelte: boolean, elements: any[] | null}} ctx
 *   `elements` collects element nodes for the in-tag comment pass, or is `null`
 *   when the document has no comments to classify.
 */
function walk_add_loc(value, ctx) {
	if (Array.isArray(value)) {
		for (const v of value) walk_add_loc(v, ctx);
	} else if (value && typeof value === 'object') {
		if (typeof value.start === 'number' && typeof value.end === 'number') {
			value.loc = { start: loc_at(value.start, ctx.starts), end: loc_at(value.end, ctx.starts) };
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
		// Re-stamp the identifiers Svelte gives the name-shaped `loc`, after the walk
		// above wrote them the plain shape.
		if (ctx.is_svelte) stamp_character_locs(value, ctx.starts, ctx.source);
	}
}

/**
 * Throw if the tree holds a block-binding type annotation whose nodes need a per-parse
 * line seed this reconstruction cannot compute.
 *
 * A Svelte block binding's `: T` is read by a **second** acorn parse that Svelte enters
 * five bytes behind the colon, on a synthetic `_ as ` which *overwrites* those bytes. So a
 * newline the author put between the binding and its colon is erased before acorn sees it,
 * and the annotation's nodes stay on the *binding's* line — a line count no single table
 * over the real source produces.
 *
 * **Leading whitespace is the exact discriminator**, and it needs no parent tracking: every
 * acorn-built `TSTypeAnnotation` opens on a token — the `:` of a parameter, variable or
 * member, or the `=>` of a function type's return — while only Svelte's block-binding one is
 * anchored at the *binding's* end, which is whitespace exactly when something separates it
 * from the colon. (Scanning ahead for a `:` instead is not a discriminator at all: a
 * function-type return annotation has no colon of its own, so the scan runs off into an
 * unrelated one far down the file.)
 *
 * Sibling of the two-line-class refusal in `create_locator`, and refused for the same
 * reason: the span-only wire records the offsets, not which parse produced them. This one
 * needs the tree rather than the source alone, which is why it lives here.
 */
function refuse_seeded_annotation(node, source) {
	if (!node || typeof node !== 'object') return;
	if (Array.isArray(node)) {
		for (const item of node) refuse_seeded_annotation(item, source);
		return;
	}
	if (node.type === 'TSTypeAnnotation' && typeof node.start === 'number') {
		let i = node.start;
		let broken = false;
		for (; i < source.length && /\s/.test(source[i]); i++) {
			if (source.charCodeAt(i) === LF) broken = true;
		}
		if (broken) {
			refuse(
				'a block binding is separated from its `: T` by a newline, and Svelte reads that ' +
					'annotation with a second acorn parse whose synthetic `_ as ` overwrites the break'
			);
		}
	}
	for (const key of Object.keys(node)) {
		if (key !== 'loc' && key !== 'name_loc') refuse_seeded_annotation(node[key], source);
	}
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
	if (is_svelte) refuse_seeded_annotation(ast, source);
	const comments = is_svelte && Array.isArray(ast?.comments) ? ast.comments : null;
	const elements = comments !== null && comments.length > 0 ? [] : null;
	walk_add_loc(ast, { starts, source, is_svelte, elements });
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
 * @param {string} source - the exact source the span-only wire was parsed from.
 * @param {{language?: 'typescript' | 'svelte' | 'css'}} [opts] - line rule
 *   selector; defaults to `typescript` (ECMAScript line terminators). Pass
 *   `svelte` for a `.svelte` document (LF-only), `css` for a no-op reconstruct.
 * @returns {{loc_of: (node: any) => ({start: {line: number, column: number}, end: {line: number, column: number}} | null), reconstruct: (ast: any) => any}}
 */
export function create_locator(source, opts) {
	const language = opts?.language ?? 'typescript';
	if (language === 'svelte' && ECMASCRIPT_ONLY_TERMINATOR.test(source)) {
		refuse(
			'the source contains a lone CR, U+2028, or U+2029, so its acorn-parsed nodes carry ' +
				'a different line count from the rest of the document'
		);
	}
	const starts = build_line_starts(source, rule_for(language));
	return {
		loc_of(node) {
			if (!node || typeof node.start !== 'number' || typeof node.end !== 'number') {
				return null;
			}
			return { start: loc_at(node.start, starts), end: loc_at(node.end, starts) };
		},
		reconstruct(ast) {
			return reconstruct_in(ast, starts, source, language);
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

/**
 * The line/column rules tsv's `loc` wire is built from, shared by the two JS modules that
 * build it: `locations.js` (the public reconstruction over a decoded or stored tree) and
 * `tape_decode.js` (the tape decoder, which builds `loc` as it decodes for
 * `parse_*(source, {locations: true})`). One statement of each rule — the text a wire's
 * offsets index, the line table per document, the line lookup, the Svelte `name_loc` span,
 * and the `character`-bearing positions Svelte's own template reader creates — so the two
 * cannot drift. `locations.js`'s module doc states the definition these implement.
 *
 * Internal: no package entry exports it, though every package that ships `locations.js`
 * ships it beside it. It imports nothing.
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
export function indexed_text(source, language) {
	if (language !== 'typescript' && source.charCodeAt(0) === BOM) return source.slice(1);
	return source;
}

/** The ECMAScript line terminators other than LF — what makes a TypeScript document's
 * line starts more than its LFs. */
const NON_LF_TERMINATOR = /[\r\u2028\u2029]/;

/**
 * Line-start offsets (UTF-16 units); the rightmost start `<=` an offset gives its
 * line. Built once per source and reused for every line lookup. A TypeScript
 * document counts the ECMAScript LineTerminators (`\n`, `\r`, `\r\n` as one, U+2028,
 * U+2029); a Svelte or CSS document counts LF alone.
 *
 * When LF is the only terminator in play — any Svelte or CSS document, and a TypeScript
 * one holding no other terminator — the starts are collected with native `indexOf`
 * rather than a per-unit scan — the common case, and the far cheaper path.
 * @param {string} source
 * @param {boolean} ecmascript
 * @returns {number[]}
 */
export function build_line_starts(source, ecmascript) {
	const starts = [0];
	if (!ecmascript || !NON_LF_TERMINATOR.test(source)) {
		for (let i = source.indexOf('\n'); i !== -1; i = source.indexOf('\n', i + 1)) {
			starts.push(i + 1);
		}
		return starts;
	}
	for (let i = 0; i < source.length; i++) {
		const c = source.charCodeAt(i);
		if (c === LF) {
			starts.push(i + 1);
		} else if (c === CR) {
			if (source.charCodeAt(i + 1) === LF) i++; // \r\n counts as one line break
			starts.push(i + 1);
		} else if (c === LINE_SEPARATOR || c === PARAGRAPH_SEPARATOR) {
			starts.push(i + 1);
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
export function line_index_at(offset, starts) {
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
 * `line_index_at`'s answer, searched outward from `hint` (any index into `starts`) rather
 * than across the whole table: the line `hint` names and the one after it are checked
 * first, then the search gallops away from `hint` — steps of 1, 2, 4, … — until it
 * brackets `offset`, and binary-searches the bracket. The walk's offsets are near-monotone
 * (a node's `start` is usually on the line of the node visited before it, its `end`
 * usually on its `start`'s line), so the answer is almost always `hint` or a neighbour,
 * and a far jump costs about twice the binary search's steps. The answer never depends
 * on `hint` — only the work does — so a stale or arbitrary hint is safe.
 * @param {number} offset
 * @param {number[]} starts
 * @param {number} hint
 * @returns {number}
 */
export function line_index_near(offset, starts, hint) {
	// `starts[0]` is 0: an offset below it (or NaN) is line 0, as the binary search answers
	if (!(offset > 0)) return 0;
	const last = starts.length - 1;
	let lo;
	let hi; // exclusive: `starts[hi] > offset`, or `hi` is past the table
	if (starts[hint] <= offset) {
		if (hint === last || offset < starts[hint + 1]) return hint;
		lo = hint + 1;
		let step = 1;
		hi = lo + 1;
		while (hi <= last && starts[hi] <= offset) {
			lo = hi;
			step += step;
			hi = lo + step;
		}
		if (hi > last + 1) hi = last + 1;
	} else {
		hi = hint;
		lo = hint - 1;
		if (starts[lo] <= offset) return lo;
		let step = 1;
		while (lo > 0 && starts[lo] > offset) {
			hi = lo;
			step += step;
			lo = hi - step;
		}
		if (lo < 0) lo = 0;
	}
	// `starts[lo] <= offset < starts[hi]`: the rightmost start `<=` offset in `[lo, hi)`
	hi--;
	while (lo < hi) {
		const mid = (lo + hi + 1) >> 1;
		if (starts[mid] <= offset) lo = mid;
		else hi = mid - 1;
	}
	return lo;
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
export const NAME_LOC_KINDS = new Map([
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
 * Whether an `Attribute` starting at `start` and named `name` is the shorthand form (`{x}`,
 * or padded: `{ x }`), whose name sits inside the braces rather than at the node start. A
 * `<script>`/`<style>` attribute name is a literal run that can itself be braced
 * (`<script {x}>`, name `{x}`), so a verbatim name at the node start is not one.
 * @param {number} start
 * @param {string} name
 * @param {string} source
 * @returns {boolean}
 */
function is_shorthand_attribute(start, name, source) {
	return source[start] === '{' && !source.startsWith(name, start);
}

/**
 * The `[start, end]` UTF-16 offsets a Svelte node's `name_loc` covers, from the node's own
 * span and `name` — or `null` for a shorthand attribute whose name the braces do not hold.
 * @param {'element' | 'directive' | 'attribute'} kind - `NAME_LOC_KINDS`' entry for the
 *   node's type
 * @param {number} start - the node's `start`
 * @param {number} end - the node's `end`
 * @param {string} name - the node's `name`
 * @param {string} source
 * @returns {[number, number] | null}
 */
function name_span(kind, start, end, name, source) {
	if (kind === 'element') {
		const at = start + 1; // the tag name follows `<`
		return [at, at + name.length];
	}
	if (kind === 'directive') {
		let stop = start;
		while (stop < end && !NAME_TERMINATORS.test(source[stop])) stop++;
		return [start, stop];
	}
	if (is_shorthand_attribute(start, name, source)) {
		// `{ x }` names just the `x` — Svelte's reader ate the padding before reading the
		// identifier. The padding is whitespace, so the first occurrence of the name at or
		// after the `{` IS the identifier, whichever whitespace chars the padding used.
		const name_start = source.indexOf(name, start + 1);
		if (name_start < 0 || name_start >= end) return null;
		return [name_start, name_start + name.length];
	}
	return [start, start + name.length];
}

/**
 * A Svelte node's `name_loc` — `{start, end}`, each `{line, column, character}` — from its
 * span and `name`, or `null` where `name_span` finds none. The lines are searched outward
 * from `hint` (`line_index_near`), the node's `start` line where the caller has it.
 * @param {'element' | 'directive' | 'attribute'} kind - `NAME_LOC_KINDS`' entry for the
 *   node's type
 * @param {number} start - the node's `start`
 * @param {number} end - the node's `end`
 * @param {string} name - the node's `name`
 * @param {string} source - the string the wire's offsets index
 * @param {number[]} starts - its line starts
 * @param {number} hint
 * @returns {{start: {line: number, column: number, character: number},
 * 	end: {line: number, column: number, character: number}} | null}
 */
export function name_loc_of(kind, start, end, name, source, starts, hint) {
	const span = name_span(kind, start, end, name, source);
	// TODO: the decoder would then emit `name_loc: null` where `locations.js` omits the key
	// (Rust always writes one); unreachable by any known source.
	if (span === null) return null;
	return { start: name_loc_near(span[0], starts, hint), end: name_loc_near(span[1], starts, hint) };
}

/**
 * The `Identifier` a shorthand attribute expands to (`{x}` → `x`), or `null`.
 * @param {any} node - a shorthand `Attribute`
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
 * A position carrying `character` — line/column plus the offset itself, the extra
 * field Svelte's `name_loc` (and the `loc` of the few nodes its own reader creates)
 * carries and a plain `loc` does not.
 * @param {number} offset
 * @param {number[]} starts
 * @returns {{line: number, column: number, character: number}}
 */
function name_loc_at(offset, starts) {
	const i = line_index_at(offset, starts);
	return { line: i + 1, column: offset - starts[i], character: offset };
}

/**
 * `name_loc_at`, its line searched outward from `hint` (`line_index_near`).
 * @param {number} offset
 * @param {number[]} starts
 * @param {number} hint
 * @returns {{line: number, column: number, character: number}}
 */
function name_loc_near(offset, starts, hint) {
	const i = line_index_near(offset, starts, hint);
	return { line: i + 1, column: offset - starts[i], character: offset };
}

/**
 * Overwrite `node`'s plain `loc` with the `character`-bearing one, if it's an identifier.
 * @param {any} node
 * @param {number[]} starts
 * @mutates node
 */
function stamp_name_shaped_loc(node, starts) {
	if (node?.type !== 'Identifier') return;
	if (typeof node.start !== 'number' || typeof node.end !== 'number') return;
	node.loc = { start: name_loc_at(node.start, starts), end: name_loc_at(node.end, starts) };
}

/** The node types `stamp_character_locs` acts on — its switch's cases. */
export const STAMP_HOSTS = new Set([
	'Attribute',
	'SnippetBlock',
	'EachBlock',
	'AwaitBlock',
	'ConstTag'
]);

/**
 * Give the identifiers under `node` the `character`-bearing `loc` Svelte reports on the
 * ones its own reader creates, rather than the plain `{line, column}` an acorn-parsed node
 * carries: a shorthand attribute's expansion (`{x}`), a snippet name, and a block pattern
 * that is a simple identifier (`{#each … as x}`, `{:then x}`, `{:catch x}`,
 * `{@const x = …}`).
 * @param {any} node
 * @param {number[]} starts
 * @param {string} source
 * @mutates node
 */
export function stamp_character_locs(node, starts, source) {
	switch (node.type) {
		case 'Attribute':
			if (
				typeof node.name === 'string' &&
				typeof node.start === 'number' &&
				is_shorthand_attribute(node.start, node.name, source)
			) {
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
 * Give each **in-tag** comment the `character`-bearing `loc` Svelte reports on it.
 *
 * Svelte's template reader collects the comments written *between* an element's
 * attributes (`<div /* c *\/ class="x">`) and stamps `character` into their `loc`;
 * every other comment is collected by acorn and gets the plain shape. Both land in the
 * root `comments` array and the wire writes both in one key order, so which collector
 * built a comment is read off the tree: an in-tag comment lies in an element's opening
 * tag (or `<svelte:options>`'s), outside every positioned value the element holds — its
 * attributes, a `<svelte:element>`'s `tag`, a `<svelte:component>`'s `expression`
 * (`in_tag_head`). Those last two exclude their braces, so an acorn comment inside
 * `this={/* c *\/ x}` sits in the same gap; its nearest neighbours past whitespace and
 * other comments tell it apart, a `{` or `(` before it or a `}` or `)` after it, which
 * no tag head puts next to an in-tag comment (`delimited_by_expression`). A template
 * comment costs one descent of the template, a `<script>` comment nothing.
 * @param {any} root
 * @param {number[]} starts
 * @param {string} source - the string the wire's offsets index
 * @mutates root
 */
export function stamp_in_tag_comment_locs(root, starts, source) {
	/** @type {{by_end: Map<number, number>, by_start: Map<number, number>} | null} */
	let neighbours = null;
	for (const c of root.comments) {
		if (typeof c?.start !== 'number' || typeof c.end !== 'number') continue;
		if (contains(root.instance?.content, c) || contains(root.module?.content, c)) continue;
		if (!in_tag_head(root, c)) continue;
		neighbours ??= comment_neighbours(root.comments);
		if (delimited_by_expression(source, c, neighbours)) continue;
		c.loc = { start: name_loc_at(c.start, starts), end: name_loc_at(c.end, starts) };
	}
}

/**
 * Whether the positioned object `node` spans `c`.
 * @param {any} node
 * @param {{start: number, end: number}} c
 */
function contains(node, c) {
	return (
		node !== null &&
		typeof node === 'object' &&
		typeof node.start === 'number' &&
		node.start <= c.start &&
		c.end <= node.end
	);
}

/**
 * Whether `c` lies in an opening tag's gaps: descend the template to the element whose
 * head holds it, `<svelte:options>` first (it sits outside the fragment).
 * @param {any} root
 * @param {{start: number, end: number}} c
 */
function in_tag_head(root, c) {
	if (contains(root.options, c)) return head_holds(root.options, c) === true;
	/** @type {any[]} */
	let fragments = [root.fragment];
	descend: for (;;) {
		for (const fragment of fragments) {
			const node = node_spanning(fragment?.nodes, c);
			if (node === null) continue;
			if (Array.isArray(node.attributes)) {
				const held = head_holds(node, c);
				if (held !== null) return held;
			}
			fragments = [];
			for (const key in node) {
				if (node[key]?.type === 'Fragment') fragments.push(node[key]);
			}
			continue descend;
		}
		return false;
	}
}

/**
 * The node of a fragment's `nodes` (sorted, disjoint) that spans `c`, or `null`.
 * @param {any[] | undefined} nodes
 * @param {{start: number, end: number}} c
 */
function node_spanning(nodes, c) {
	if (!Array.isArray(nodes)) return null;
	let lo = 0;
	let hi = nodes.length - 1;
	while (lo <= hi) {
		const mid = (lo + hi) >> 1;
		const node = nodes[mid];
		if (node.end < c.start) lo = mid + 1;
		else if (node.start > c.start) hi = mid - 1;
		else return contains(node, c) ? node : null;
	}
	return null;
}

/**
 * For an element spanning `c`: `true` when `c` is in its head outside every positioned
 * value, `false` when a value holds it, `null` when it lies past the head (look deeper).
 * @param {any} node
 * @param {{start: number, end: number}} c
 * @returns {boolean | null}
 */
function head_holds(node, c) {
	const head_end = node.fragment?.nodes?.[0]?.start ?? node.end;
	if (c.end > head_end) return null;
	for (const key in node) {
		if (key === 'fragment' || key === 'loc' || key === 'name_loc') continue;
		const value = node[key];
		if (Array.isArray(value)) {
			for (const item of value) if (contains(item, c)) return false;
		} else if (contains(value, c)) {
			return false;
		}
	}
	return true;
}

/**
 * Every comment's `start` by its `end`, and `end` by `start` — how
 * `delimited_by_expression` steps over a run of comments.
 * @param {any[]} comments
 */
function comment_neighbours(comments) {
	const by_end = new Map();
	const by_start = new Map();
	for (const c of comments) {
		by_end.set(c.end, c.start);
		by_start.set(c.start, c.end);
	}
	return { by_end, by_start };
}

/**
 * Whether `c`'s nearest neighbours, past whitespace and other comments, are an
 * expression's delimiters: a `{` or `(` before it, or a `}` or `)` after it.
 * @param {string} source
 * @param {{start: number, end: number}} c
 * @param {{by_end: Map<number, number>, by_start: Map<number, number>}} neighbours
 */
function delimited_by_expression(source, c, { by_end, by_start }) {
	let before = c.start;
	for (;;) {
		while (before > 0 && /\s/.test(source[before - 1])) before--;
		const start = by_end.get(before);
		if (start === undefined) break;
		before = start;
	}
	if (source[before - 1] === '{' || source[before - 1] === '(') return true;
	let after = c.end;
	for (;;) {
		while (after < source.length && /\s/.test(source[after])) after++;
		const end = by_start.get(after);
		if (end === undefined) break;
		after = end;
	}
	return source[after] === '}' || source[after] === ')';
}

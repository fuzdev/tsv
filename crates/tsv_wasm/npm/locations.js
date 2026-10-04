/**
 * Line/column reconstruction for tsv's span-only parse wire.
 *
 * Every tsv parse emits a span-only AST: every node carries its `start`/`end` (UTF-16
 * code-unit offsets) and no per-node `loc` (line/column) object (Svelte also no
 * element/attribute/directive `name_loc`). Line/column is a pure function of an offset
 * plus the source, so a consumer recovers both on demand — no re-parse. This module is
 * that reconstruction, shipped so callers don't reimplement the line rules; a package's
 * `parse_*(source, {locations: true})` runs it for them.
 *
 * Two entry points:
 * - `reconstruct_locations(ast, source, options?)` — one-shot: build the line table once,
 *   walk the tree, add `loc` to every node (and `name_loc` back to the Svelte nodes that
 *   carry one), return the (mutated) ast. The language is inferred from the root when
 *   `options.language` is omitted — a whole parse's root (`Root`, `StyleSheetFile`, or a
 *   `Program` spanning the whole source); any other node names no document, a Svelte
 *   `<script>`'s `Program` included, so it throws rather than guess.
 * - `create_locator(source, {language})` — amortized: build the line table once and
 *   expose `position_at(offset)` (one offset), `loc_of(node)` (one node) and
 *   `reconstruct(ast)` (whole tree) over it, for any number of lookups.
 *
 * `create_locator` takes the language **required** — a bare source names no document, and
 * the language decides the line rule, the BOM, and the Svelte stamping, so a default would
 * silently answer for the wrong document.
 *
 * **Errors.** A bad argument — a `source` that is not a string, a missing or unknown
 * `language`, a root whose language cannot be inferred, an offset handed to `position_at`
 * that is not a number, a node handed to `loc_of` whose `start` or `end` getter throws —
 * throws a `TypeError`. A numeric offset outside the indexed text
 * (or not an integer) throws a `RangeError` from a locator's two single lookups
 * (`position_at`, `loc_of`), which check what they are handed; `loc_of` answers `null` for
 * a value without numeric `start` and `end`, as the walk skips one. The whole-tree walk
 * (`reconstruct`, `reconstruct_locations`) is the hot path and checks no node — it trusts
 * the tree to be a parse of `source`, and, like any walk of it, to be acyclic.
 *
 * Every parse-capable package also exports this module alone, as its `./locations`
 * subpath: it imports nothing, so that entry loads no engine.
 *
 * **The definition.** Every object in the tree that carries numeric `start` and `end` —
 * in all three languages, objects without a `type` included (Svelte's `StyleSheet.content`
 * and `options`, comments) — gets `loc: {start: {line, column}, end: {line, column}}`: the
 * line (1-based) and column (0-based, UTF-16 code units) of that object's own `start` and
 * `end`. One line-terminator rule per **document**: ECMAScript's (LF, CR, CRLF as one,
 * U+2028, U+2029) for a TypeScript document, LF alone for a Svelte document — its
 * `<script>`s, template expressions and `<style>` included — and for a CSS document. That
 * is the same definition tsv's Rust `loc` emitter is written to (`tsv parse --locations`),
 * so the two agree on every object: the reconstruction of the span-only wire deep-equals
 * the loc wire of the same parse. (Only key order differs — this module appends `loc`
 * last on each object, where the Rust wire places it after `end` — so a deep-equal sees
 * identical data, and a re-serialized tree won't byte-match that wire.)
 *
 * The TypeScript count is acorn's. A Svelte document's is not Svelte's everywhere: Svelte's
 * own wire carries `loc` only on the nodes acorn parsed (this adds it to template, style and
 * option objects too), counts ECMAScript terminators on those nodes, puts a `<script>`
 * program's `loc` at the tag rather than at the content its `start`/`end` name, and reads a
 * block binding's pattern or type annotation a column or line off in a few spellings. Every
 * one of those is a fact about how Svelte's parser is assembled, not about the source, and
 * none is reproduced here.
 *
 * **Svelte `name_loc` and `character`.** Svelte's wire carries a `name_loc` on every
 * element, attribute and directive, and a `character` field (the offset itself) in the
 * position objects of the few nodes its own template reader creates. Both are exact
 * functions of a node's span and type, so the walk restores them:
 * - `name_loc` (`{line, column, character}` endpoints): a tag name is the run after `<`, an
 *   attribute name starts at the node (a shorthand `{x}` names the identifier inside the
 *   braces, so `{ x }` excludes the padding), and a directive names its whole head token
 *   (`on:click|preventDefault`).
 * - `character` in `loc`: a shorthand attribute's expansion (`{x}`), a snippet name, a
 *   simple-identifier block pattern (`{#each … as x}`, `{:then x}`, `{:catch x}`,
 *   `{@const x = …}`), and an **in-tag** comment — one written between an element's
 *   attributes, which Svelte's template reader collects rather than acorn — told apart by
 *   the key order each collector writes (see `stamp_in_tag_comment_locs`).
 *
 * **A leading byte-order mark is read the way each wire reads it.** Svelte's `parse` and
 * `parseCss` strip a U+FEFF at index 0 before parsing, so the Svelte and CSS wires index
 * the BOM-less string; acorn counts it as whitespace, so the TypeScript wire indexes the
 * caller's string as given. The line table and every name span are built over the string
 * the wire's offsets index — `source` with its BOM dropped for Svelte and CSS, `source`
 * itself for TypeScript — so a consumer hands over the source it parsed, BOM and all.
 *
 * `reconstruct_locations` and a locator's `reconstruct` **mutate the ast in place** (adding a `loc`
 * key to each node) and return it, for efficiency on large trees. Callers that
 * need the input untouched should `structuredClone(ast)` first. The tree must be
 * **acyclic** — the parse's own tree, or a structured clone of it: the walk keeps no
 * visited set, so a tree a caller has given back-pointers (`parent`) never finishes.
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

/** The ECMAScript line terminators other than LF — what makes a TypeScript document's
 * line starts more than its LFs. */
const NON_LF_TERMINATOR = /[\r\u2028\u2029]/;

/**
 * Line-start offsets (UTF-16 units); the rightmost start `<=` an offset gives its
 * line. Built once per source and reused for every `loc_at` lookup. A TypeScript
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
function build_line_starts(source, ecmascript) {
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
 * Read the language off the AST root when `options.language` is omitted:
 * `Root` → svelte, `Program` → typescript, `StyleSheetFile` → css. `undefined` for
 * anything else — a subtree (a Svelte `Fragment`, a statement) names no document, and
 * guessing would silently apply the wrong line rule and skip Svelte's `name_loc`.
 *
 * A `Program` is a TypeScript root only when it spans the whole source, as a parse's
 * root always does: a Svelte `<script>`'s program (`instance.content`) is a `Program`
 * too, keyed identically, but it starts after its tag — read as a TypeScript root it
 * would count ECMAScript terminators and a BOM the Svelte document doesn't.
 * @param {any} ast
 * @param {string} source
 * @returns {'typescript' | 'svelte' | 'css' | undefined}
 */
function infer_language(ast, source) {
	if (ast && typeof ast === 'object') {
		switch (ast.type) {
			case 'Root':
				return 'svelte';
			case 'Program':
				return ast.start === 0 && ast.end === source.length ? 'typescript' : undefined;
			case 'StyleSheetFile':
				return 'css';
		}
	}
	return undefined;
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
 * @param {'element' | 'directive' | 'attribute' | undefined} kind - `NAME_LOC_KINDS`' entry
 *   for the node's type, which the walk has already looked up
 * @param {string} source
 * @returns {[number, number] | null}
 */
function name_span_of(node, kind, source) {
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
const STAMP_HOSTS = new Set(['Attribute', 'SnippetBlock', 'EachBlock', 'AwaitBlock', 'ConstTag']);

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
 * Give each **in-tag** comment the `character`-bearing `loc` Svelte reports on it.
 *
 * Svelte's template reader collects the comments written *between* an element's
 * attributes (`<div /* c *\/ class="x">`) and stamps `character` into their `loc`;
 * every other comment is collected by acorn and gets the plain shape. The wire itself
 * records which collector built each one, in its key order — the template reader's
 * literal is `{type, start, end, value}`, acorn's `onComment` wrapper's
 * `{type, value, start, end}` — so that order is read here, not re-derived from the
 * tree. It holds for any tree a parse returned (`JSON.parse` and `structuredClone` keep
 * key order); a tree rebuilt with its comments' keys reordered loses the stamp.
 * @param {any[]} comments
 * @param {number[]} starts
 * @mutates comments
 */
function stamp_in_tag_comment_locs(comments, starts) {
	for (const c of comments) {
		if (typeof c?.start !== 'number' || typeof c.end !== 'number') continue;
		if (Object.keys(c)[1] !== 'start') continue;
		c.loc = { start: name_loc_at(c.start, starts), end: name_loc_at(c.end, starts) };
	}
}

/**
 * Walk `root`, adding a `loc` object to every object with numeric `start`/`end` —
 * and, for a Svelte tree, a `name_loc` to every element, attribute, and directive
 * that carries one. Mutates in place. Skips the keys it writes so it never
 * re-walks its own output.
 *
 * An explicit stack, not recursion: a recursive walk spends a JS frame per nesting
 * level and meets the host's JS stack (a `RangeError`) well before the parser's own
 * depth ceiling — on the N-API addon, whose parse runs on the host's native stack, a
 * few thousand nested elements in. Depth here costs only the stack array.
 *
 * The `character`-bearing identifiers Svelte stamps are re-stamped after the walk
 * (`stamp_character_locs`, per collected `STAMP_HOSTS` node), once the plain shape the
 * walk writes every object has landed on them. Nothing reads the visit order.
 * @param {any} root
 * @param {{starts: number[], source: string, is_svelte: boolean}} ctx
 */
function walk_add_loc(root, ctx) {
	const { starts, source, is_svelte } = ctx;
	if (root === null || typeof root !== 'object') return;
	const stack = [root];
	const stamp_hosts = is_svelte ? [] : null;
	while (stack.length > 0) {
		const value = stack.pop();
		if (Array.isArray(value)) {
			for (const v of value) if (v !== null && typeof v === 'object') stack.push(v);
			continue;
		}
		if (typeof value.start === 'number' && typeof value.end === 'number') {
			value.loc = { start: loc_at(value.start, starts), end: loc_at(value.end, starts) };
			if (is_svelte) {
				const kind = NAME_LOC_KINDS.get(value.type);
				const span = name_span_of(value, kind, source);
				if (span) {
					value.name_loc = {
						start: name_loc_at(span[0], starts),
						end: name_loc_at(span[1], starts)
					};
				}
			}
		}
		for (const key in value) {
			if (key === 'loc' || key === 'name_loc') continue;
			const v = value[key];
			if (v !== null && typeof v === 'object') stack.push(v);
		}
		if (stamp_hosts !== null && STAMP_HOSTS.has(value.type)) stamp_hosts.push(value);
	}
	if (stamp_hosts !== null) {
		for (const host of stamp_hosts) stamp_character_locs(host, starts, source);
	}
}

/**
 * The whole reconstruction for one already-built line table: the `loc`/`name_loc`
 * walk plus the Svelte in-tag comment pass. Shared by both entry points so they
 * can't drift.
 * @param {any} ast
 * @param {number[]} starts
 * @param {string} source
 * @param {boolean} is_svelte
 * @returns {any} the same `ast`, mutated
 */
function reconstruct_in(ast, starts, source, is_svelte) {
	walk_add_loc(ast, { starts, source, is_svelte });
	if (is_svelte && Array.isArray(ast?.comments)) stamp_in_tag_comment_locs(ast.comments, starts);
	return ast;
}

/** The languages a locator reads, each its own line rule. */
const LANGUAGES = new Set(['typescript', 'svelte', 'css']);

/**
 * Refuse a `source` that is not a string — every entry point reads it as text (its length,
 * its line terminators), and a number or `null` would otherwise index as a document of no
 * lines. `api.js`'s `read_source` states the same rule for the parse and format exports;
 * restated here rather than imported: this module imports nothing, so the `./locations`
 * subpath stays one self-contained file.
 *
 * @param {unknown} source - the caller's source
 * @returns {string} the same `source`
 * @throws TypeError when `source` is not a string
 */
function read_locations_source(source) {
	if (typeof source !== 'string') {
		throw new TypeError(
			`locations source must be a string (got ${source === null ? 'null' : typeof source})`
		);
	}
	return source;
}

/**
 * Read the `language` off an options bag, by `api.js`'s `read_options` rules: `undefined` /
 * `null` mean no bag, any other non-object (arrays included) is refused, and so is a key
 * other than `language` — a typo must not silently fall back to inference. The bag is read
 * by its own enumerable keys and nothing else, so a `language` it inherits is not read. The
 * value itself is graded by `create_locator`.
 *
 * @param {unknown} options - the caller's bag
 * @returns {unknown} the bag's `language`, `undefined` when unset
 * @throws TypeError when `options` is not an object, carries an unknown key, or its
 *   `language` getter throws (the getter's error rides along as the `cause`)
 */
function read_language_option(options) {
	if (options === undefined || options === null) return undefined;
	if (typeof options !== 'object' || Array.isArray(options)) {
		throw new TypeError('locations options must be an object');
	}
	let language;
	for (const name of Object.keys(options)) {
		if (name !== 'language') {
			throw new TypeError(`unknown locations option ${quote_string(name)} (expected 'language')`);
		}
		try {
			language = /** @type {{language?: unknown}} */ (options).language;
		} catch (cause) {
			throw new TypeError("failed to read locations option 'language'", { cause });
		}
	}
	return language;
}

/**
 * Read one offset off a node handed to `loc_of`, once — the node is the caller's, so a
 * getter that throws is named rather than escaping raw, as the option readers name theirs.
 *
 * @param {object} node - the caller's node
 * @param {'start' | 'end'} key
 * @returns {unknown}
 * @throws TypeError when the key's getter throws (the getter's error rides along as the
 *   `cause`)
 */
function read_node_offset(node, key) {
	try {
		return /** @type {Record<string, unknown>} */ (node)[key];
	} catch (cause) {
		throw new TypeError(`loc_of: failed to read the node's '${key}'`, { cause });
	}
}

/**
 * Whether `offset` is a position in a text of `length` UTF-16 units: an integer from 0
 * through `length` (the end of the text is a position — a node ending there names it).
 *
 * @param {unknown} offset
 * @param {number} length
 * @returns {boolean}
 */
function is_offset_in(offset, length) {
	return (
		Number.isInteger(offset) &&
		/** @type {number} */ (offset) >= 0 &&
		/** @type {number} */ (offset) <= length
	);
}

/**
 * A refused value as an error message's `(got …)` names it: a string quoted, a number or
 * boolean as written, `null`, `none` for `undefined`, and anything else by its `typeof`.
 * A string is echoed as a single-quoted literal with JSON's escapes plus the terminal-unsafe
 * characters JSON leaves raw (so a line break, a control, a bidi control or a lone surrogate
 * never reaches the message raw) and clipped past `DESCRIBED_STRING_MAX` UTF-16 units with a
 * `…` inside the quotes. `api.js` states the
 * same rule; restated here rather than imported, like `read_locations_source`.
 *
 * @param {unknown} value - the refused value
 * @returns {string}
 */
function describe_value(value) {
	if (typeof value === 'string') return quote_string(value);
	if (typeof value === 'number' || typeof value === 'boolean') return String(value);
	if (value === undefined) return 'none';
	return value === null ? 'null' : typeof value;
}

/** The longest refused string `describe_value` echoes whole, in UTF-16 units. */
const DESCRIBED_STRING_MAX = 40;

/**
 * `value` as a single-quoted literal, escaped as `JSON.stringify` escapes (with `'` escaped
 * and `"` not, for the other quote) plus `\uXXXX` for what JSON leaves raw but a terminal
 * reads as a line break, a control or a reordering (`TERMINAL_UNSAFE`), clipped to
 * `DESCRIBED_STRING_MAX` units plus `…` — one unit sooner where the cut would split a
 * surrogate pair.
 *
 * @param {string} value
 * @returns {string}
 */
function quote_string(value) {
	let cut = value.length;
	if (cut > DESCRIBED_STRING_MAX) {
		cut = DESCRIBED_STRING_MAX;
		const last = value.charCodeAt(cut - 1);
		if (last >= 0xd800 && last <= 0xdbff) cut--;
	}
	const body = JSON.stringify(value.slice(0, cut))
		.slice(1, -1)
		.replace(/\\"/g, '"')
		.replace(/'/g, "\\'")
		.replace(TERMINAL_UNSAFE, (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`);
	return `'${body}${cut < value.length ? '…' : ''}'`;
}

/** The characters JSON leaves raw that a terminal reads as a line break (NEL, LS, PS), a
 * C1 control or DEL, or a bidi control. */
const TERMINAL_UNSAFE = /[\u007f-\u009f\u061c\u2028\u2029\u200e\u200f\u202a-\u202e\u2066-\u2069]/g;

/**
 * Build a locator that holds the source's line-start table, so any number of lookups
 * against one source build it once. For a whole tree, `reconstruct_locations` builds it
 * once too; the locator is for lookups one offset or one node at a time.
 *
 * Offsets are in the wire's coordinates: UTF-16 units into the text the parse's offsets
 * index — `source` itself for TypeScript, `source` without a leading BOM for Svelte and
 * CSS. `position_at` refuses an offset that is not a number with a `TypeError`, and
 * `position_at` and `loc_of` refuse a numeric offset that is not an integer within that
 * text with a `RangeError`; `loc_of` answers `null` for a value without numeric `start`
 * and `end`, reads each once, and names a getter that throws with a `TypeError`.
 * `reconstruct` checks nothing, and takes an acyclic tree (see the module doc).
 *
 * @param {string} source - the exact source the span-only wire was parsed from
 * @param {{language: 'typescript' | 'svelte' | 'css'}} options - `language` (required)
 *   selects the document's line rule and coordinates: `typescript` (ECMAScript line
 *   terminators, a leading BOM counted), `svelte` or `css` (LF alone, a leading BOM elided).
 * @returns {{
 * 	position_at: (offset: number) => {line: number, column: number},
 * 	loc_of: (node: {start?: number | undefined, end?: number | undefined} | null | undefined) =>
 * 		({start: {line: number, column: number}, end: {line: number, column: number}} | null),
 * 	reconstruct: (ast: any) => any
 * }}
 * @throws TypeError when `source` is not a string, `options` is not an object or carries
 *   a key other than `language`, or `options.language` is missing or not one of the three
 */
export function create_locator(source, options) {
	read_locations_source(source);
	const language = read_language_option(options);
	if (!LANGUAGES.has(language)) {
		throw new TypeError(
			"locations option 'language' must be 'typescript', 'svelte' or 'css' " +
				`(got ${describe_value(language)})`
		);
	}
	// Everything below reads the string the wire's offsets index (see `indexed_text`),
	// never the caller's `source` directly — a Svelte or CSS BOM is not in the wire's
	// coordinates.
	const text = indexed_text(source, language);
	const starts = build_line_starts(text, language === 'typescript');
	const is_svelte = language === 'svelte';
	return {
		position_at(offset) {
			if (typeof offset !== 'number') {
				throw new TypeError(`position_at: offset must be a number (got ${describe_value(offset)})`);
			}
			if (!is_offset_in(offset, text.length)) {
				throw new RangeError(
					`position_at: offset must be an integer from 0 to ${text.length}, the indexed ` +
						`text's length (got ${describe_value(offset)})`
				);
			}
			// `+ 0` folds a `-0` offset, which would otherwise surface as `column: -0`
			return loc_at(offset + 0, starts);
		},
		loc_of(node) {
			if (!node) return null;
			const start = read_node_offset(node, 'start');
			if (typeof start !== 'number') return null;
			const end = read_node_offset(node, 'end');
			if (typeof end !== 'number') return null;
			if (!is_offset_in(start, text.length) || !is_offset_in(end, text.length) || start > end) {
				throw new RangeError(
					`loc_of: node span ${start}..${end} is not a range of the indexed text ` +
						`(integer offsets from 0 to ${text.length}, start no later than end)`
				);
			}
			return { start: loc_at(start + 0, starts), end: loc_at(end + 0, starts) };
		},
		reconstruct(ast) {
			return reconstruct_in(ast, starts, text, is_svelte);
		}
	};
}

/**
 * Add a `loc: {start, end}` line/column object to every object of a span-only wire
 * that carries numeric `start`/`end`, derived from those offsets + `source` — and, for
 * a Svelte tree, the `name_loc` its elements, attributes, and directives carry. Builds
 * the line-start table once, **mutates `ast` in place**, and returns it.
 * `structuredClone(ast)` first if you need the input untouched.
 *
 * The result deep-equals the loc-bearing wire of the same parse — see the module doc.
 * The walk is the hot path and checks no offset: a tree that is not a parse of `source`
 * gets positions computed from offsets the text may not have. The tree must be acyclic —
 * the parse's own tree, or a structured clone of it; one with back-pointers never finishes.
 *
 * @param {any} ast - a span-only AST, as every tsv parse returns it by default
 * @param {string} source - the exact source `ast` was parsed from
 * @param {{language?: 'typescript' | 'svelte' | 'css'}} [options] - the document's
 *   language; inferred from the root node (`Root`/`Program`/`StyleSheetFile`) when omitted.
 * @returns {any} the same `ast`, now with `loc` on every node (plus `name_loc` on
 *   the Svelte nodes that carry one).
 * @throws TypeError when `source` is not a string, `options` is not an object or carries a
 *   key other than `language`, `options.language` is omitted and `ast` is not one of those
 *   three roots, or it is set and not one of the three languages
 */
export function reconstruct_locations(ast, source, options) {
	read_locations_source(source);
	// an unset `language` infers; a set one — `null` included — is graded by `create_locator`
	const named = read_language_option(options);
	const language = named === undefined ? infer_language(ast, source) : named;
	if (language === undefined) {
		const type = ast && typeof ast === 'object' ? ast.type : undefined;
		const root =
			type === 'Program'
				? "a 'Program' that does not span the source"
				: typeof type === 'string'
					? `a '${type}' root`
					: 'a root with no type';
		throw new TypeError(
			"locations option 'language' is required: cannot infer the document's language " +
				`from ${root} — ` +
				`pass {language: 'typescript' | 'svelte' | 'css'}, or the parse's own root ` +
				`(Root, Program or StyleSheetFile)`
		);
	}
	return create_locator(source, { language }).reconstruct(ast);
}

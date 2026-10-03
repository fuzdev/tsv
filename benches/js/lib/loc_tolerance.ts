/**
 * The `loc` arm's tolerance list: the six named ways Svelte's own `loc` departs from tsv's
 * definition, recognized by STRUCTURE so a seventh fails.
 *
 * tsv's `loc` follows one definition — the line and UTF-16 column of each object's own
 * `start`/`end`, LF-only for a whole Svelte document (`crates/tsv_wasm/npm/locations.js`'s
 * module doc). Svelte's wire departs from it in six places, none reproduced and each
 * cataloged in docs/conformance_svelte.md §Svelte Template Corrections:
 *
 * 1. `destructure_column` — a destructured block binding (`{#each … as {…}}`, `{:then}`,
 *    `{:catch}`, `{@const}`) whose pattern opens off line 1 reads one column right on that
 *    opening line, its nodes and the comments inside it alike (`read_pattern` parses
 *    `(pattern = 1)` and drops one blank to compensate, which lands only on line 1).
 * 2. `annotation_swallow` — a newline in the four UTF-16 units before a block binding's `:`
 *    is overwritten by `read_type_annotation`'s synthetic `_ as `, so the annotation's
 *    nodes sit that many lines higher, and on the colon's line measure their column from
 *    the line the rewrite began on.
 * 3. `two_line_classes` — acorn-parsed nodes count ECMAScript's terminators (a lone CR,
 *    U+2028, U+2029) where the rest of the document counts LF alone.
 * 4. `program_at_tag` — `read_script` stamps a `<script>`'s `Program.loc` at the tag while
 *    its `start`/`end` are the content's.
 * 5. `typed_destructure_end` — a typed destructured block binding's `end` is widened over
 *    the annotation while its `loc.end` stays at the bracket.
 * 6. `each_as_stale_loc` — under `lang="ts"`, an `{#each}` expression's `end` is unwound
 *    from the `as` expression while its `loc.end` is not.
 *
 * Each row is a predicate over the canonical tree, the source and the difference itself,
 * and each requires the oracle to hold a value Svelte's model gives — so a row cannot
 * excuse a difference somewhere else, or a different difference at the same place. Rows
 * 1, 2, 4 and 5 compute that value exactly. Row 6 requires the oracle's point to name the
 * end of the `as` type Svelte's acorn read swallowed: past the `as` keyword, at or before
 * the context's end, on a token boundary. Row 3 is exact for a `<script>`'s nodes and its
 * comments (Svelte hands acorn the document with every unit ahead of the content blanked
 * but `\n`, so the count is the LF lines before the content plus every ECMAScript
 * terminator inside it) and a band for a template island, where Svelte hands acorn a
 * prefix the comparator does not model: the oracle's line must lie between the LF count
 * and the full ECMAScript count from the document start, and its column must be the LF
 * column or one measured from a lone terminator on the same LF line — evaluated at the
 * offsets row 5 and row 6 read too, admitting row 1's +1 column inside a row-1 slot and
 * row 2's swallowed lines under a row-2 swallow, since those rows compose with it. The
 * band's residual is deliberate: an island position one line off inside the band reads
 * as tolerated, because naming the exact line needs the island's own start — a per-island
 * model of the prefix Svelte prepares, which the comparator deliberately does not carry.
 *
 * Every row also requires tsv's own value to be the definition's, so no row can excuse a
 * tsv bug that lands where a quirk does. The rows apply to Svelte documents only: a
 * TypeScript document's `loc` is graded exactly against acorn, and CSS has no oracle
 * `loc` at all.
 *
 * The caller grades `loc` only where tsv's and the oracle's `start`/`end` agree (a span
 * difference is the span arm's business), so every row may read the owner's offsets off
 * the canonical tree.
 *
 * The module also holds the loc arm's other rule, the **superset**: a tsv `loc` on an
 * object the oracle gives none is accepted only for the object kinds `LOC_SUPERSET_KEYS`
 * names, so an oracle that stops writing `loc` on a node it used to carry fails rather
 * than reading as one more superset object.
 *
 * @module
 */

/** One of the six tolerance rows. */
export type LocRow =
	| 'destructure_column'
	| 'annotation_swallow'
	| 'two_line_classes'
	| 'program_at_tag'
	| 'typed_destructure_end'
	| 'each_as_stale_loc';

/** The rows in table order — the order the comparator's summary prints them in. */
export const LOC_ROWS: readonly LocRow[] = [
	'destructure_column',
	'annotation_swallow',
	'two_line_classes',
	'program_at_tag',
	'typed_destructure_end',
	'each_as_stale_loc'
];

/** The nearest enclosing object whose `type` is a pinned superset type, and its path. */
export interface SupersetAnchor {
	type: string;
	path: string;
}

/**
 * The kind a superset object is pinned by. An object whose `type` the pinned set names
 * plainly — a node only Svelte's own reader (or `parseCss`) builds, never acorn — is keyed
 * by that `type`. Any other object is keyed by its SHAPE, `Anchor.path:Type`: the nearest
 * enclosing object of a plainly pinned type, the path from it (array indices dropped) and
 * the object's `type` (empty for an untyped one) — `BindDirective.expression:Identifier`
 * for a shorthand's identifier, `ConstTag.declaration:VariableDeclaration`,
 * `StyleSheet.content:`. The shape is what keeps an acorn-shaped node Svelte builds itself
 * apart from the acorn node of the same `type` that carries a `loc`: an `{expr}` island's
 * identifier keys as `ExpressionTag.expression:Identifier`, a `<script>`'s as a path under
 * `Script.content`, neither of which is pinned. An attached comment is keyed by its list
 * alone (`leadingComments[]:Line`), whatever node it hangs off.
 *
 * The path alone cannot tell a slot's two spellings apart: `bind:value` and
 * `bind:value={foo}` both put an `Identifier` at `BindDirective.expression`, the first
 * built by Svelte (no `loc`), the second parsed by acorn (a `loc`) — and `class:a` /
 * `class:a={on}`, `this="div"` / `this={"div"}` likewise (`ISLAND_SLOTS`). So an object in
 * one of those slots whose first non-whitespace unit before its `start` is a `{` — the
 * acorn island's opening brace — keys with its type braced
 * (`BindDirective.expression:{Identifier}`), which nothing pins: an oracle that dropped
 * the acorn node's `loc` fails rather than reading as one more shorthand. The test is
 * scoped to those slots because a brace is no island elsewhere: `{ @const a = b}` puts
 * the Svelte-built declaration's `start` two units past its tag's `{`.
 *
 * @param object - an object carrying numeric `start` / `end` and a tsv `loc`.
 * @param path - the object's own diff path (`css.content`, `fragment.nodes[0]`).
 * @param anchor - the nearest enclosing object of a plainly pinned type, or `null`.
 * @param pinned - the language's `LOC_SUPERSET_KEYS`.
 * @param text - the source the wire's offsets index (`wire_text`).
 */
export function superset_key(
	object: Record<string, unknown>,
	path: string,
	anchor: SupersetAnchor | null,
	pinned: ReadonlySet<string>,
	text: string
): string {
	const type = typeof object.type === 'string' ? object.type : '';
	const tail = path.slice(path.lastIndexOf('.') + 1).replace(/\[\d+\]/g, '[]');
	// a comment's `type` (`Line` / `Block`) is no Svelte node's, so it is keyed by its list
	if ((type === 'Line' || type === 'Block') && typeof object.value === 'string') {
		return `${tail}:${type}`;
	}
	if (type !== '' && pinned.has(type)) return type;
	if (anchor === null) return `${path.replace(/\[\d+\]/g, '[]')}:${type}`;
	const relative = path.slice(anchor.path === '' ? 0 : anchor.path.length + 1);
	const slot = `${anchor.type}.${relative.replace(/\[\d+\]/g, '[]')}`;
	const braced = ISLAND_SLOTS.has(slot) && follows_brace(object.start as number, text);
	return `${slot}:${braced ? `{${type}}` : type}`;
}

/**
 * The slots Svelte fills two ways — a node it builds from the directive or attribute
 * itself, or the acorn node of a `{…}` value — so their superset key needs the spelling.
 */
const ISLAND_SLOTS: ReadonlySet<string> = new Set([
	'BindDirective.expression',
	'ClassDirective.expression',
	'SvelteElement.tag'
]);

/** Whether the first non-whitespace unit before `start` is a `{`. */
function follows_brace(start: number, text: string): boolean {
	let i = start - 1;
	while (i >= 0 && /\s/.test(text[i])) i--;
	return i >= 0 && text[i] === '{';
}

/**
 * The source a Svelte or CSS wire's offsets index: the caller's, minus a leading BOM,
 * which Svelte's `parse` and `parseCss` strip before parsing.
 */
export function wire_text(source: string): string {
	return source.charCodeAt(0) === 0xfeff ? source.slice(1) : source;
}

/**
 * The superset kinds (`superset_key`) that may carry a tsv `loc` the oracle gives none, per
 * language — a "may only contain" set, not an exact one: a run over a narrowed root meets
 * a subset, and no single run meets them all (the fixture tree and its injected variants
 * reach shapes the corpus never writes, and the corpus element/attribute mixes the
 * fixtures never do). A superset object outside the set is an undocumented difference, so
 * an oracle that stops writing `loc` on a node it used to carry fails instead of reading
 * as one more superset object. TypeScript has no set: acorn gives every node a `loc`, so
 * any tsv `loc` it lacks is a difference.
 *
 * Svelte's set is two lists. The plain `type`s are the nodes only Svelte's own reader
 * builds — template, `Root`, `<script>` / `<style>` and every CSS node in the latter,
 * attributes and directives. The shapes are the acorn-shaped nodes Svelte builds itself
 * rather than taking from acorn — a directive's shorthand identifier, `<svelte:element>`'s
 * static `this` (each slot's braced, acorn-parsed spelling keys apart — `superset_key`),
 * a `{@const}`'s declaration, a block binding's annotation — plus every
 * attached comment and the two untyped spans. CSS's is every node `parseCss` writes, since
 * it writes no `loc` at all. Add a kind only for an object the oracle has never given a
 * `loc`; a kind that loses one it used to carry is the regression this set exists to
 * catch.
 */
const LOC_SUPERSET_KEYS: Record<'svelte' | 'css', ReadonlySet<string>> = {
	svelte: new Set([
		// the nodes Svelte's own reader builds
		'AnimateDirective',
		'Atrule',
		'AttachTag',
		'Attribute',
		'AttributeSelector',
		'AwaitBlock',
		'BindDirective',
		'Block',
		'CSSComment',
		'ClassDirective',
		'ClassSelector',
		'Combinator',
		'Comment',
		'ComplexSelector',
		'Component',
		'ConstTag',
		'DebugTag',
		'Declaration',
		'DeclarationTag',
		'EachBlock',
		'ExpressionTag',
		'HtmlTag',
		'IdSelector',
		'IfBlock',
		'KeyBlock',
		'LetDirective',
		'NestingSelector',
		'Nth',
		'OnDirective',
		'Percentage',
		'PseudoClassSelector',
		'PseudoElementSelector',
		'RegularElement',
		'RelativeSelector',
		'RenderTag',
		'Root',
		'Rule',
		'Script',
		'SelectorList',
		'SlotElement',
		'SnippetBlock',
		'SpreadAttribute',
		'StyleDirective',
		'StyleSheet',
		'SvelteBody',
		'SvelteBoundary',
		'SvelteComponent',
		'SvelteDocument',
		'SvelteElement',
		'SvelteFragment',
		'SvelteHead',
		'SvelteSelf',
		'SvelteWindow',
		'Text',
		'TitleElement',
		'TransitionDirective',
		'TypeSelector',
		'UseDirective',
		// the acorn-shaped nodes Svelte builds itself, attached comments, untyped spans
		'AwaitBlock.error.typeAnnotation:TSTypeAnnotation',
		'AwaitBlock.value.typeAnnotation:TSTypeAnnotation',
		'BindDirective.expression:Identifier',
		'ClassDirective.expression:Identifier',
		'ConstTag.declaration.declarations[].id.typeAnnotation:TSTypeAnnotation',
		'ConstTag.declaration.declarations[]:VariableDeclarator',
		'ConstTag.declaration:VariableDeclaration',
		'EachBlock.context.typeAnnotation:TSTypeAnnotation',
		'Root.options:',
		'StyleSheet.content:',
		'SvelteElement.tag:Literal',
		'leadingComments[]:Block',
		'leadingComments[]:Line',
		'trailingComments[]:Block',
		'trailingComments[]:Line'
	]),
	css: new Set([
		'Atrule',
		'AttributeSelector',
		'Block',
		'CSSComment',
		'ClassSelector',
		'Combinator',
		'ComplexSelector',
		'Declaration',
		'IdSelector',
		'NestingSelector',
		'Nth',
		'Percentage',
		'PseudoClassSelector',
		'PseudoElementSelector',
		'RelativeSelector',
		'Rule',
		'SelectorList',
		'StyleSheetFile',
		'TypeSelector'
	])
};

/** The language's pinned superset kinds, or `null` for TypeScript, which has none. */
export function superset_kinds_of(language: LocLanguage): ReadonlySet<string> | null {
	return language === 'typescript' ? null : LOC_SUPERSET_KEYS[language];
}

/** A `loc` leaf difference's path: the owner object, the side and the field. */
const LOC_LEAF = /^(?:(.*)\.)?loc\.(start|end)\.(line|column)$/;

/** Whether `path` names a `loc` line or column — the leaves only the rows may classify. */
export function is_loc_leaf(path: string): boolean {
	return LOC_LEAF.test(path);
}

/** Resolve a node by concrete diff path (`fragment.nodes[3].expression`); `''` is the root. */
export function get_at_path(root: unknown, path: string): unknown {
	if (path === '') return root;
	let node = root;
	for (const seg of path.split('.')) {
		if (node == null) return null;
		const m = seg.match(/^([^[]+)((?:\[\d+\])*)$/);
		if (!m) return null;
		node = (node as Record<string, unknown>)[m[1]];
		for (const idx of m[2].matchAll(/\[(\d+)\]/g)) {
			if (!Array.isArray(node)) return null;
			node = node[Number(idx[1])];
		}
	}
	return node;
}

type Node = Record<string, any>;

/** A block-binding slot: the pattern Svelte's own `read_pattern` reads, and its path. */
interface Slot {
	path: string;
	pattern: Node;
}

/** Per-document facts the rows share, built once on the first `loc` difference. */
interface Document {
	/** The source the Svelte wire indexes — the caller's, minus a leading BOM. */
	text: string;
	/** LF line starts over `text`: Svelte's `locate-character` table. */
	starts: number[];
	/**
	 * Where each lone CR / U+2028 / U+2029 in `text` sits, ascending — the terminators
	 * ECMAScript counts and the LF table does not.
	 */
	ecmascript_only: number[];
	/** Every block-binding slot in the canonical tree, for the root `comments` list. */
	slots: Slot[] | null;
}

/** A document's language, as the comparator names it. */
export type LocLanguage = 'svelte' | 'typescript' | 'css';

/** What the rows read about one file — the comparator's own match context. */
export interface LocRowContext {
	source: string;
	canonical_root: unknown;
	/** Only a Svelte document's differences can fall in a row. */
	language: LocLanguage;
}

const documents = new WeakMap<LocRowContext, Document>();

function document_of(ctx: LocRowContext): Document {
	let doc = documents.get(ctx);
	if (doc === undefined) {
		const text = wire_text(ctx.source);
		const starts = [0];
		for (let i = 0; i < text.length; i++) if (text.charCodeAt(i) === 0x0a) starts.push(i + 1);
		const ecmascript_only: number[] = [];
		for (const m of text.matchAll(/\r(?!\n)|[\u2028\u2029]/g)) ecmascript_only.push(m.index);
		doc = { text, starts, ecmascript_only, slots: null };
		documents.set(ctx, doc);
	}
	return doc;
}

/** 0-based line index of `offset` in an LF table. */
function line_index(offset: number, starts: number[]): number {
	let lo = 0;
	let hi = starts.length - 1;
	while (lo < hi) {
		const mid = (lo + hi + 1) >> 1;
		if (starts[mid] <= offset) lo = mid;
		else hi = mid - 1;
	}
	return lo;
}

/** The definition's point for `offset`: 1-based line, 0-based column, LF-only. */
function point(offset: number, starts: number[]): { line: number; column: number } {
	const i = line_index(offset, starts);
	return { line: i + 1, column: offset - starts[i] };
}

/** How many of the ascending `positions` lie below `offset`. */
function count_below(offset: number, positions: number[]): number {
	let lo = 0;
	let hi = positions.length;
	while (lo < hi) {
		const mid = (lo + hi) >> 1;
		if (positions[mid] < offset) lo = mid + 1;
		else hi = mid;
	}
	return lo;
}

/**
 * The point acorn gives `offset` inside a `<script>` whose content starts at
 * `content_start`. Svelte hands acorn the whole document with every unit ahead of the
 * content blanked but `\n`, so the lines before the content are LF lines, and inside it
 * every ECMAScript terminator opens one.
 */
function script_point(
	offset: number,
	content_start: number,
	doc: Document
): { line: number; column: number } {
	const lf = line_index(offset, doc.starts);
	const before_content = count_below(content_start, doc.ecmascript_only);
	const below = count_below(offset, doc.ecmascript_only);
	const last_terminator = below > before_content ? doc.ecmascript_only[below - 1] : -1;
	const line_start = Math.max(doc.starts[lf], last_terminator + 1);
	return { line: lf + 1 + below - before_content, column: offset - line_start };
}

/**
 * Whether `(line, column)` is a point acorn can give `offset` in a template island, over
 * any prefix Svelte hands it: the line between the LF count and the full ECMAScript count
 * from the document start (less `line_slack`, row 2's swallowed lines), the column the LF
 * column or one measured from a lone terminator on the same LF line (or one more, under
 * `column_slack`, row 1's shift).
 */
function in_island_band(
	offset: number,
	line: number,
	column: number,
	doc: Document,
	column_slack: 0 | 1,
	line_slack: number
): boolean {
	const lf = line_index(offset, doc.starts);
	const below = count_below(offset, doc.ecmascript_only);
	if (line < lf + 1 - line_slack || line > lf + 1 + below) return false;
	const columns = [offset - doc.starts[lf]];
	for (let k = count_below(doc.starts[lf], doc.ecmascript_only); k < below; k++) {
		columns.push(offset - (doc.ecmascript_only[k] + 1));
	}
	return columns.some((c) => column === c || column === c + column_slack);
}

/** The `<script>` content holding the owner at `owner_path`, if it sits in one. */
function script_content_of(owner_path: string, owner: Node, root: Node): Node | null {
	const m = /^(instance|module)\.content\./.exec(owner_path);
	if (m) return (root[m[1]] as Node | null)?.content ?? null;
	if (!/^comments\[\d+\]$/.test(owner_path)) return null;
	for (const key of ['instance', 'module']) {
		const content = (root[key] as Node | null)?.content as Node | undefined;
		if (
			typeof content?.start === 'number' &&
			owner.start >= content.start &&
			owner.end <= content.end
		) {
			return content;
		}
	}
	return null;
}

/** The slot that `path` lies in (or is), with the rest of the path below it. */
function find_slot(path: string, root: unknown): { slot: Slot; rest: string } | null {
	const segs = path === '' ? [] : path.split('.');
	let node = root as Node | null;
	let prefix = '';
	const join = (seg: string) => (prefix === '' ? seg : `${prefix}.${seg}`);
	for (let i = 0; i < segs.length; i++) {
		const seg = segs[i];
		let slot_path: string | null = null;
		let consumed = 1;
		if (node?.type === 'EachBlock' && seg === 'context') slot_path = join(seg);
		else if (node?.type === 'AwaitBlock' && (seg === 'value' || seg === 'error')) {
			slot_path = join(seg);
		} else if (
			node?.type === 'ConstTag' &&
			seg === 'declaration' &&
			/^declarations\[\d+\]$/.test(segs[i + 1] ?? '') &&
			segs[i + 2] === 'id'
		) {
			slot_path = join(`declaration.${segs[i + 1]}.id`);
			consumed = 3;
		}
		if (slot_path !== null) {
			const pattern = get_at_path(root, slot_path) as Node | null;
			if (pattern == null || typeof pattern !== 'object') return null;
			const rest = segs.slice(i + consumed).join('.');
			return { slot: { path: slot_path, pattern }, rest };
		}
		prefix = join(seg);
		node = get_at_path(node, seg) as Node | null;
	}
	return null;
}

/** Every block-binding slot in the canonical tree (cached on the document). */
function slots_of(ctx: LocRowContext, doc: Document): Slot[] {
	if (doc.slots !== null) return doc.slots;
	const out: Slot[] = [];
	const walk = (value: unknown, path: string): void => {
		if (Array.isArray(value)) {
			value.forEach((v, i) => walk(v, `${path}[${i}]`));
			return;
		}
		if (value === null || typeof value !== 'object') return;
		const node = value as Node;
		const at = (key: string) => (path === '' ? key : `${path}.${key}`);
		const push = (slot_path: string, pattern: unknown) => {
			if (pattern !== null && typeof pattern === 'object') {
				out.push({ path: slot_path, pattern: pattern as Node });
			}
		};
		if (node.type === 'EachBlock') push(at('context'), node.context);
		if (node.type === 'AwaitBlock') {
			push(at('value'), node.value);
			push(at('error'), node.error);
		}
		if (node.type === 'ConstTag' && Array.isArray(node.declaration?.declarations)) {
			node.declaration.declarations.forEach((d: Node, i: number) =>
				push(at(`declaration.declarations[${i}].id`), d?.id)
			);
		}
		for (const [key, child] of Object.entries(node)) {
			if (key !== 'loc' && key !== 'name_loc') walk(child, at(key));
		}
	};
	walk(ctx.canonical_root, '');
	doc.slots = out;
	return out;
}

const is_destructure = (pattern: Node): boolean =>
	pattern.type === 'ObjectPattern' || pattern.type === 'ArrayPattern';

/** Where a destructure pattern's own text ends: its annotation's start, or its end. */
const bare_end = (pattern: Node): number =>
	typeof pattern.typeAnnotation?.start === 'number' ? pattern.typeAnnotation.start : pattern.end;

/** The `_ as ` rewrite a block binding's annotation went through, if it erased a newline. */
interface Swallow {
	colon: number;
	annotation_end: number;
	erased: number;
	colon_line: number;
	base_line: number;
}

function swallow_of(pattern: Node, doc: Document): Swallow | null {
	const annotation = pattern.typeAnnotation as Node | undefined;
	if (typeof annotation?.start !== 'number' || typeof annotation.end !== 'number') return null;
	// the colon is where Svelte's `allow_whitespace` stops, as its reader found it
	let colon = annotation.start;
	while (colon < doc.text.length && /\s/.test(doc.text[colon])) colon++;
	if (doc.text[colon] !== ':') return null;
	// `_ as ` overwrites the five units ending at the colon: four before it
	const window_start = Math.max(0, colon - 4);
	let erased = 0;
	for (let i = window_start; i < colon; i++) if (doc.text.charCodeAt(i) === 0x0a) erased++;
	if (erased === 0) return null;
	return {
		colon,
		annotation_end: annotation.end,
		erased,
		colon_line: line_index(colon, doc.starts),
		base_line: line_index(window_start, doc.starts)
	};
}

/** The point Svelte's annotation parse gives `offset` under `s`. */
function swallowed_point(offset: number, s: Swallow, starts: number[]) {
	const i = line_index(offset, starts);
	if (i === s.colon_line) return { line: s.base_line + 1, column: offset - starts[s.base_line] };
	return { line: i + 1 - s.erased, column: offset - starts[i] };
}

/**
 * The row a `loc` leaf difference falls in, or `null` — an unclassified difference, which
 * the comparator fails on.
 *
 * @param path - the leaf's concrete path (`fragment.nodes[0].context.loc.start.column`).
 * @param ours - tsv's value at the leaf.
 * @param canonical - the oracle's value at the leaf.
 */
export function classify_loc_difference(
	path: string,
	ours: unknown,
	canonical: unknown,
	ctx: LocRowContext
): LocRow | null {
	if (ctx.language !== 'svelte') return null;
	const m = LOC_LEAF.exec(path);
	if (!m || typeof ours !== 'number' || typeof canonical !== 'number') return null;
	const owner_path = m[1] ?? '';
	const side = m[2] as 'start' | 'end';
	const field = m[3] as 'line' | 'column';
	const owner = get_at_path(ctx.canonical_root, owner_path) as Node | null;
	const offset = owner?.[side];
	if (typeof offset !== 'number') return null;
	const doc = document_of(ctx);
	// no row excuses a tsv value that is not the definition's
	if (ours !== point(offset, doc.starts)[field]) return null;

	const found = find_slot(owner_path, ctx.canonical_root);
	const is_root_comment = /^comments\[\d+\]$/.test(owner_path);

	// 1. destructure_column
	if (field === 'column' && canonical === ours + 1) {
		const pattern_line = (pattern: Node) => line_index(pattern.start, doc.starts);
		const shifted = (pattern: Node): boolean =>
			is_destructure(pattern) &&
			pattern_line(pattern) > 0 &&
			line_index(offset, doc.starts) === pattern_line(pattern);
		if (
			found !== null &&
			!found.rest.startsWith('typeAnnotation') &&
			offset <= bare_end(found.slot.pattern) &&
			shifted(found.slot.pattern)
		) {
			return 'destructure_column';
		}
		if (
			is_root_comment &&
			slots_of(ctx, doc).some(
				({ pattern }) =>
					shifted(pattern) && owner!.start >= pattern.start && owner!.end <= bare_end(pattern)
			)
		) {
			return 'destructure_column';
		}
	}

	// 2. annotation_swallow
	const swallowed = (pattern: Node): boolean => {
		const s = swallow_of(pattern, doc);
		return s !== null && canonical === swallowed_point(offset, s, doc.starts)[field];
	};
	if (found !== null && found.rest.startsWith('typeAnnotation.') && swallowed(found.slot.pattern)) {
		return 'annotation_swallow';
	}
	if (is_root_comment) {
		for (const { pattern } of slots_of(ctx, doc)) {
			const s = swallow_of(pattern, doc);
			if (s !== null && owner!.start > s.colon && owner!.end <= s.annotation_end) {
				if (swallowed(pattern)) return 'annotation_swallow';
			}
		}
	}

	// 4. program_at_tag
	const program = /^(instance|module)\.content$/.exec(owner_path);
	if (program) {
		const script = (ctx.canonical_root as Node)[program[1]] as Node | null;
		const at = script?.[side];
		if (typeof at === 'number' && canonical === point(at, doc.starts)[field]) {
			return 'program_at_tag';
		}
	}

	// 5. typed_destructure_end
	if (found !== null && found.rest === '' && side === 'end') {
		const pattern = found.slot.pattern;
		const annotation = pattern.typeAnnotation as Node | undefined;
		if (
			is_destructure(pattern) &&
			typeof annotation?.start === 'number' &&
			pattern.end === annotation.end
		) {
			const p = point(annotation.start, doc.starts);
			const start_line = line_index(pattern.start, doc.starts) + 1;
			// the bracket inherits row 1's shift when it closes on an opening line past line 1
			if (start_line > 1 && p.line === start_line) p.column += 1;
			if (canonical === p[field]) return 'typed_destructure_end';
		}
	}

	// 6. each_as_stale_loc — the `{#each}` expression's `loc.end`, which Svelte leaves at
	// the end of the `as` type its acorn read swallowed: the oracle's point must sit past
	// the `as` keyword, at or before the context's end, on a token boundary (the `as` test
	// is what bounds it below — a point at or before `offset` reads an empty slice)
	const oracle = oracle_point(owner!, side, field, canonical);
	const each_block = side === 'end' ? each_block_of(owner_path, ctx.canonical_root) : null;
	const context_end = each_block?.context?.end;
	if (oracle !== null && typeof context_end === 'number') {
		const at = offset_at(oracle, doc.starts);
		if (
			at !== null &&
			at <= context_end &&
			/^\s*as\s+\S/.test(doc.text.slice(offset, at)) &&
			(at === context_end || /[\s,}():]/.test(doc.text[at] ?? ''))
		) {
			return 'each_as_stale_loc';
		}
	}

	// 3. two_line_classes — last, and only on an acorn-counted position behind a terminator
	// the two classes disagree on (a position carrying `character` was counted by Svelte's
	// own LF locator, so it is never one)
	if (oracle === null || oracle.character !== undefined) return null;
	if (doc.ecmascript_only.length === 0 || offset <= doc.ecmascript_only[0]) return null;
	// a `<script>`'s node or comment: exact
	const content = script_content_of(owner_path, owner!, ctx.canonical_root as Node);
	if (content !== null && content.start <= offset) {
		const p = script_point(offset, content.start, doc);
		return p.line === oracle.line && p.column === oracle.column ? 'two_line_classes' : null;
	}
	// a template island: the band, at every offset a row composing with this one reads
	const offsets = [offset];
	let column_slack: 0 | 1 = 0;
	let line_slack = 0;
	if (found !== null) {
		const pattern = found.slot.pattern;
		const in_annotation = found.rest.startsWith('typeAnnotation');
		// row 5: the slot's `loc.end` at its annotation's start
		if (found.rest === '' && side === 'end' && typeof pattern.typeAnnotation?.start === 'number') {
			offsets.push(pattern.typeAnnotation.start);
		}
		// row 1: one column right inside a destructure
		if (is_destructure(pattern) && !in_annotation) column_slack = 1;
		// row 2: the lines the `_ as ` rewrite erased
		if (in_annotation) line_slack = swallow_of(pattern, doc)?.erased ?? 0;
	}
	// row 6: the `{#each}` expression's `loc.end` past its `as`
	if (typeof context_end === 'number') offsets.push(context_end, bare_end(each_block!.context));
	return offsets.some((o) =>
		in_island_band(o, oracle.line, oracle.column, doc, column_slack, line_slack)
	)
		? 'two_line_classes'
		: null;
}

/** The oracle's point at the owner's `side`, with the differing `field` read as `canonical`. */
function oracle_point(
	owner: Node,
	side: 'start' | 'end',
	field: 'line' | 'column',
	canonical: number
): { line: number; column: number; character?: number } | null {
	const p = (owner.loc as Node | undefined)?.[side] as Node | undefined;
	if (typeof p?.line !== 'number' || typeof p.column !== 'number') return null;
	return { line: p.line, column: p.column, character: p.character, [field]: canonical };
}

/** The offset an LF-counted `(line, column)` names, or `null` past the table. */
function offset_at(p: { line: number; column: number }, starts: number[]): number | null {
	if (p.line < 1 || p.line > starts.length) return null;
	return starts[p.line - 1] + p.column;
}

/** The `{#each}` block whose `expression` `owner_path` is, if it is one. */
function each_block_of(owner_path: string, root: unknown): Node | null {
	const each = /^(.*)\.expression$/.exec(owner_path);
	if (!each) return null;
	const block = get_at_path(root, each[1]) as Node | null;
	return block?.type === 'EachBlock' ? block : null;
}

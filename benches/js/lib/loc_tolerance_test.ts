/**
 * Tests for the `loc` arm's tolerance rows (`classify_loc_difference`) and its superset
 * pin (`superset_key` / `superset_kinds_of`).
 *
 * Each row gets the difference it exists for, which must classify as that row, and
 * mutants of it that must NOT classify at all — the same shape one step off (a column
 * two right where Svelte's model gives one, a line past the band), and the row's value
 * on a neighbor's owner (a program-shaped `loc` on a statement, a destructure's shift on a
 * plain identifier, the each-`as` end on an expression that is no `{#each}`'s). A row
 * that admits a mutant is a row that would excuse a tsv bug landing there. The rows apply
 * to Svelte only: a TypeScript document's `loc` is graded exactly and a CSS document's has
 * no oracle, so no difference in either classifies.
 *
 * Hand-written canonical trees holding only the fields the rows read, on purpose: the
 * rows are predicates over a tree's shape, and a test that parsed through the oracle
 * would need the sidecar and stop gating in `deno task check` (see `test:deno`). What
 * the real oracle writes is graded by `corpus:compare:parse`.
 */

import { strictEqual } from 'node:assert';
import {
	classify_loc_difference,
	get_at_path,
	type LocLanguage,
	type LocRow,
	superset_key,
	superset_kinds_of
} from './loc_tolerance.ts';

type Node = Record<string, any>;
type Point = { line: number; column: number; character?: number };

/** The definition's point for `offset`: LF lines, UTF-16 column. */
function lf_point(source: string, offset: number): Point {
	const before = source.slice(0, offset);
	return { line: before.split('\n').length, column: offset - (before.lastIndexOf('\n') + 1) };
}

/** A node spanning `source[start, end)`, its `loc` the definition's. */
function node(source: string, type: string | null, start: number, end: number, rest = {}): Node {
	const typed = type === null ? {} : { type };
	return {
		...typed,
		start,
		end,
		loc: { start: lf_point(source, start), end: lf_point(source, end) },
		...rest
	};
}

/** The offset of the `n`th (0-based) occurrence of `needle` in `source`. */
function at(source: string, needle: string, n = 0): number {
	let i = -1;
	for (let k = 0; k <= n; k++) {
		i = source.indexOf(needle, i + 1);
		if (i === -1) throw Error(`no ${needle} #${n} in ${JSON.stringify(source)}`);
	}
	return i;
}

/**
 * Classify the oracle holding `oracle` at the `loc` point `leaf` names (its other field
 * kept, unless `oracle` names both), with tsv's value the definition's unless `ours`
 * says otherwise.
 */
function classify(
	source: string,
	root: Node,
	leaf: string,
	oracle: Partial<Point>,
	options: { ours?: number; language?: LocLanguage } = {}
): LocRow | null {
	const m = /^(?:(.*)\.)?loc\.(start|end)\.(line|column)$/.exec(leaf);
	if (!m) throw Error(`not a loc leaf: ${leaf}`);
	const side = m[2] as 'start' | 'end';
	const field = m[3] as 'line' | 'column';
	const canonical_root = structuredClone(root);
	const owner = get_at_path(canonical_root, m[1] ?? '') as Node;
	const ours = options.ours ?? lf_point(source, owner[side])[field];
	Object.assign(owner.loc[side], oracle);
	const canonical = owner.loc[side][field];
	return classify_loc_difference(leaf, ours, canonical, {
		source,
		canonical_root,
		language: options.language ?? 'svelte'
	});
}

/** A Svelte root around template `nodes`. */
const root_of = (nodes: Node[], rest = {}): Node => ({
	type: 'Root',
	fragment: { type: 'Fragment', nodes },
	...rest
});

// --- 1. destructure_column ---------------------------------------------------------

/** `{#each xs as <pattern>}{a}{/each}` behind `prefix`, the context typed by `context`. */
function each_with_context(prefix: string, pattern: string, context: string): [string, Node] {
	const source = `${prefix}{#each xs as ${pattern}}{a}{/each}`;
	const p = at(source, pattern);
	const each = node(source, 'EachBlock', prefix.length, source.length, {
		expression: node(source, 'Identifier', at(source, 'xs'), at(source, 'xs') + 2),
		context: node(source, context, p, p + pattern.length)
	});
	return [source, root_of([each])];
}

const CONTEXT_START_COLUMN = 'fragment.nodes[0].context.loc.start.column';

Deno.test('destructure_column: a destructure off line 1 reads one column right', () => {
	const [source, root] = each_with_context('\n', '{ a, b }', 'ObjectPattern');
	const column = lf_point(source, at(source, '{ a')).column;
	strictEqual(
		classify(source, root, CONTEXT_START_COLUMN, { column: column + 1 }),
		'destructure_column'
	);
});

Deno.test('destructure_column: mutants do not classify', () => {
	const [source, root] = each_with_context('\n', '{ a, b }', 'ObjectPattern');
	const column = lf_point(source, at(source, '{ a')).column;
	strictEqual(classify(source, root, CONTEXT_START_COLUMN, { column: column + 2 }), null);
	strictEqual(classify(source, root, CONTEXT_START_COLUMN, { column: column - 1 }), null);
	// tsv's own value off the definition is never excused
	strictEqual(
		classify(source, root, CONTEXT_START_COLUMN, { column: column + 1 }, { ours: column - 1 }),
		null
	);
	// on line 1 Svelte's compensation lands: no shift to excuse
	const [line_one, line_one_root] = each_with_context('', '{ a, b }', 'ObjectPattern');
	const first = lf_point(line_one, at(line_one, '{ a')).column;
	strictEqual(classify(line_one, line_one_root, CONTEXT_START_COLUMN, { column: first + 1 }), null);
	// a plain identifier binding is no destructure
	const [plain, plain_root] = each_with_context('\n', 'item', 'Identifier');
	const item = lf_point(plain, at(plain, 'item')).column;
	strictEqual(classify(plain, plain_root, CONTEXT_START_COLUMN, { column: item + 1 }), null);
});

Deno.test('destructure_column: a root comment inside the pattern, and only inside it', () => {
	const source = '\n{#each xs as { a /* c */ }}{a}{/each} /* d */';
	const p = at(source, '{ a');
	const each = node(source, 'EachBlock', 1, at(source, '{/each}') + 7, {
		expression: node(source, 'Identifier', at(source, 'xs'), at(source, 'xs') + 2),
		context: node(source, 'ObjectPattern', p, at(source, '}}') + 1)
	});
	const comment = (needle: string) =>
		node(source, 'Block', at(source, needle), at(source, needle) + needle.length, {
			value: needle.slice(2, -2)
		});
	const root = root_of([each], { comments: [comment('/* c */'), comment('/* d */')] });
	const inside = lf_point(source, at(source, '/* c */')).column;
	strictEqual(
		classify(source, root, 'comments[0].loc.start.column', { column: inside + 1 }),
		'destructure_column'
	);
	const outside = lf_point(source, at(source, '/* d */')).column;
	strictEqual(
		classify(source, root, 'comments[1].loc.start.column', { column: outside + 1 }),
		null
	);
});

// --- 2. annotation_swallow ---------------------------------------------------------

/** `{#each xs as x<gap>: T}{x}{/each}`, the annotation starting at its colon. */
function each_with_annotation(gap: string): [string, Node] {
	const source = `{#each xs as x${gap}: T}{x}{/each}`;
	const x = at(source, 'x', 1);
	const colon = at(source, ':');
	const annotation = node(source, 'TSTypeAnnotation', colon, colon + 3, {
		typeAnnotation: node(source, 'TSTypeReference', colon + 2, colon + 3)
	});
	const each = node(source, 'EachBlock', 0, source.length, {
		expression: node(source, 'Identifier', at(source, 'xs'), at(source, 'xs') + 2),
		context: node(source, 'Identifier', x, x + 1, { typeAnnotation: annotation })
	});
	return [source, root_of([each])];
}

const ANNOTATION_TYPE = 'fragment.nodes[0].context.typeAnnotation.typeAnnotation.loc.start';

Deno.test('annotation_swallow: the `_ as ` rewrite erases the newline before the colon', () => {
	const [source, root] = each_with_annotation('\n');
	const t = at(source, 'T');
	// the type sits on the colon's line: one line up, its column measured from line 1
	strictEqual(
		classify(source, root, `${ANNOTATION_TYPE}.line`, { line: 1, column: t }),
		'annotation_swallow'
	);
	strictEqual(
		classify(source, root, `${ANNOTATION_TYPE}.column`, { line: 1, column: t }),
		'annotation_swallow'
	);
});

Deno.test('annotation_swallow: mutants do not classify', () => {
	const [source, root] = each_with_annotation('\n');
	const t = at(source, 'T');
	strictEqual(
		classify(source, root, `${ANNOTATION_TYPE}.column`, { line: 1, column: t + 1 }),
		null
	);
	strictEqual(classify(source, root, `${ANNOTATION_TYPE}.line`, { line: 0, column: t }), null);
	// the binding itself is not in the annotation: no swallow reaches it
	strictEqual(classify(source, root, 'fragment.nodes[0].context.loc.end.line', { line: 2 }), null);
	// a newline outside the rewrite's window erases nothing
	const [wide, wide_root] = each_with_annotation('\n    ');
	const wide_t = at(wide, 'T');
	strictEqual(
		classify(wide, wide_root, `${ANNOTATION_TYPE}.line`, { line: 1, column: wide_t }),
		null
	);
});

// --- 3. two_line_classes -----------------------------------------------------------

const SCRIPT = '<script>\nlet a = 1;\u2028let b = 2;\n</script>\n';

function script_root(source: string): Node {
	const content_start = at(source, '>') + 1;
	const content_end = at(source, '</script>');
	const declaration = (needle: string) =>
		node(source, 'VariableDeclaration', at(source, needle), at(source, needle) + needle.length);
	return {
		type: 'Root',
		fragment: { type: 'Fragment', nodes: [] },
		instance: node(source, 'Script', 0, content_end + 9, {
			content: node(source, 'Program', content_start, content_end, {
				body: [declaration('let a = 1;'), declaration('let b = 2;')]
			})
		})
	};
}

Deno.test(
	'two_line_classes: a script node counts every ECMAScript terminator in the script',
	() => {
		const root = script_root(SCRIPT);
		const leaf = 'instance.content.body[1].loc.start';
		strictEqual(classify(SCRIPT, root, `${leaf}.line`, { line: 3, column: 0 }), 'two_line_classes');
		strictEqual(
			classify(SCRIPT, root, `${leaf}.column`, { line: 3, column: 0 }),
			'two_line_classes'
		);
	}
);

Deno.test('two_line_classes: a script node is exact — mutants do not classify', () => {
	const root = script_root(SCRIPT);
	const leaf = 'instance.content.body[1].loc.start';
	strictEqual(classify(SCRIPT, root, `${leaf}.line`, { line: 4, column: 0 }), null);
	strictEqual(classify(SCRIPT, root, `${leaf}.column`, { line: 3, column: 1 }), null);
	// the LF column on the ECMAScript line is neither count's point
	const lf_column = lf_point(SCRIPT, at(SCRIPT, 'let b')).column;
	strictEqual(classify(SCRIPT, root, `${leaf}.line`, { line: 3, column: lf_column }), null);
	// no terminator ahead of the position: nothing for the two counts to disagree on
	strictEqual(classify(SCRIPT, root, 'instance.content.body[0].loc.start.line', { line: 3 }), null);
});

/** `<p>\u2028{a + b}</p>`: an island behind a lone LS, on one LF line. */
function island(): [string, Node] {
	const source = '<p>\u2028{a + b}</p>';
	const b = at(source, 'b');
	const tag = node(source, 'ExpressionTag', at(source, '{'), at(source, '}') + 1, {
		expression: node(source, 'BinaryExpression', at(source, 'a'), b + 1, {
			left: node(source, 'Identifier', at(source, 'a'), at(source, 'a') + 1),
			right: node(source, 'Identifier', b, b + 1)
		})
	});
	const p = node(source, 'RegularElement', 0, source.length, {
		fragment: { type: 'Fragment', nodes: [node(source, 'Text', 3, 4), tag] }
	});
	return [source, root_of([p])];
}

const ISLAND_B = 'fragment.nodes[0].fragment.nodes[1].expression.right.loc.start';

Deno.test('two_line_classes: an island point within the band classifies', () => {
	const [source, root] = island();
	const b = at(source, 'b');
	const ls = at(source, '\u2028');
	strictEqual(
		classify(source, root, `${ISLAND_B}.line`, { line: 2, column: b - ls - 1 }),
		'two_line_classes'
	);
	strictEqual(
		classify(source, root, `${ISLAND_B}.column`, { line: 2, column: b - ls - 1 }),
		'two_line_classes'
	);
});

Deno.test('two_line_classes: island mutants outside the band do not classify', () => {
	const [source, root] = island();
	const b = at(source, 'b');
	const ls = at(source, '\u2028');
	// past the full ECMAScript count
	strictEqual(classify(source, root, `${ISLAND_B}.line`, { line: 3, column: b - ls - 1 }), null);
	// a column measured from no terminator
	strictEqual(classify(source, root, `${ISLAND_B}.column`, { line: 2, column: b - ls }), null);
	// a point Svelte's own LF locator wrote (it carries `character`) is never acorn's
	strictEqual(
		classify(source, root, `${ISLAND_B}.line`, { line: 2, column: b - ls - 1, character: b }),
		null
	);
	// a CRLF is one terminator to both counts
	const crlf = '<p>\r\n{a}</p>';
	const a = at(crlf, 'a');
	const crlf_root = root_of([
		node(crlf, 'RegularElement', 0, crlf.length, {
			fragment: {
				type: 'Fragment',
				nodes: [
					node(crlf, 'Text', 3, 5),
					node(crlf, 'ExpressionTag', a - 1, a + 2, {
						expression: node(crlf, 'Identifier', a, a + 1)
					})
				]
			}
		})
	]);
	strictEqual(
		classify(crlf, crlf_root, 'fragment.nodes[0].fragment.nodes[1].expression.loc.start.line', {
			line: 3
		}),
		null
	);
});

// --- 4. program_at_tag -------------------------------------------------------------

const PROGRAM = '<div></div>\n\t<script>\nlet a;\n</script>';

Deno.test('program_at_tag: a script Program reads its tag position', () => {
	const root = script_root(PROGRAM.replace('let a;', 'let a = 1;let b = 2;'));
	const source = PROGRAM.replace('let a;', 'let a = 1;let b = 2;');
	const tag = lf_point(source, root.instance.start);
	strictEqual(classify(source, root, 'instance.content.loc.start.column', tag), 'program_at_tag');
	const tag_end = lf_point(source, root.instance.end);
	strictEqual(classify(source, root, 'instance.content.loc.end.column', tag_end), 'program_at_tag');
});

Deno.test('program_at_tag: mutants do not classify', () => {
	const source = PROGRAM.replace('let a;', 'let a = 1;let b = 2;');
	const root = script_root(source);
	const tag = lf_point(source, root.instance.start);
	strictEqual(
		classify(source, root, 'instance.content.loc.start.column', { column: tag.column + 1 }),
		null
	);
	// the tag's point on a statement is no Program's quirk
	strictEqual(classify(source, root, 'instance.content.body[0].loc.start.column', tag), null);
	// tsv's own value off the definition is never excused, though the oracle's is the tag's
	const definition = lf_point(source, root.instance.content.start).column;
	strictEqual(
		classify(source, root, 'instance.content.loc.start.column', tag, { ours: definition + 1 }),
		null
	);
});

// --- 5. typed_destructure_end ------------------------------------------------------

/** `{#each xs as { a }: T}…` behind `prefix`, the pattern's `end` widened over `: T`. */
function typed_destructure(prefix: string): [string, Node, number] {
	const source = `${prefix}{#each xs as { a }: T}{a}{/each}`;
	const p = at(source, '{ a');
	const colon = at(source, ':');
	const pattern = node(source, 'ObjectPattern', p, colon + 3, {
		typeAnnotation: node(source, 'TSTypeAnnotation', colon, colon + 3)
	});
	const each = node(source, 'EachBlock', prefix.length, source.length, {
		expression: node(source, 'Identifier', at(source, 'xs'), at(source, 'xs') + 2),
		context: pattern
	});
	return [source, root_of([each]), colon];
}

const CONTEXT_END_COLUMN = 'fragment.nodes[0].context.loc.end.column';

Deno.test('typed_destructure_end: loc.end stays at the bracket', () => {
	const [source, root, colon] = typed_destructure('');
	strictEqual(
		classify(source, root, CONTEXT_END_COLUMN, lf_point(source, colon)),
		'typed_destructure_end'
	);
	// off line 1 the bracket also carries row 1's shift
	const [off, off_root, off_colon] = typed_destructure('\n');
	const shifted = lf_point(off, off_colon);
	shifted.column += 1;
	strictEqual(classify(off, off_root, CONTEXT_END_COLUMN, shifted), 'typed_destructure_end');
});

Deno.test('typed_destructure_end: mutants do not classify', () => {
	const [source, root, colon] = typed_destructure('');
	const p = lf_point(source, colon);
	strictEqual(classify(source, root, CONTEXT_END_COLUMN, { column: p.column + 1 }), null);
	// off line 1, the unshifted bracket is not Svelte's value
	const [off, off_root, off_colon] = typed_destructure('\n');
	strictEqual(classify(off, off_root, CONTEXT_END_COLUMN, lf_point(off, off_colon)), null);
	// an untyped destructure's end is never widened
	const [plain, plain_root] = each_with_context('', '{ a }', 'ObjectPattern');
	strictEqual(
		classify(plain, plain_root, CONTEXT_END_COLUMN, lf_point(plain, at(plain, 'a }'))),
		null
	);
});

// --- 6. each_as_stale_loc ----------------------------------------------------------

/** `{#each contents ?? [] as section}{section}{/each}`, the expression unwound to `[]`. */
function each_as(source = '{#each contents ?? [] as section}{section}{/each}'): [string, Node] {
	const s = at(source, 'section');
	const each = node(source, 'EachBlock', 0, source.length, {
		expression: node(source, 'LogicalExpression', at(source, 'contents'), at(source, '[]') + 2),
		context: node(source, 'Identifier', s, s + 7)
	});
	const t = at(source, '{section}');
	const tag = node(source, 'ExpressionTag', t, t + 9, {
		expression: node(source, 'Identifier', t + 1, t + 8)
	});
	return [source, root_of([each, tag])];
}

const EACH_EXPRESSION_END = 'fragment.nodes[0].expression.loc.end.column';

Deno.test('each_as_stale_loc: loc.end stays at the swallowed `as` type', () => {
	const [source, root] = each_as();
	const context_end = at(source, 'section') + 7;
	strictEqual(
		classify(source, root, EACH_EXPRESSION_END, lf_point(source, context_end)),
		'each_as_stale_loc'
	);
});

Deno.test('each_as_stale_loc: mutants do not classify', () => {
	const [source, root] = each_as();
	const s = at(source, 'section');
	// past the context
	strictEqual(classify(source, root, EACH_EXPRESSION_END, lf_point(source, s + 8)), null);
	// past the context on a token boundary, the `as` test passing: the context bound alone
	const [spaced, spaced_root] = each_as('{#each contents ?? [] as section }{section}{/each}');
	const past = at(spaced, 'section') + 8;
	strictEqual(classify(spaced, spaced_root, EACH_EXPRESSION_END, lf_point(spaced, past)), null);
	// inside the context on a token boundary, but no `as <type>` read: the `as` test alone
	strictEqual(
		classify(source, root, EACH_EXPRESSION_END, lf_point(source, at(source, ' as') + 3)),
		null
	);
	// before the `as` keyword
	strictEqual(
		classify(source, root, EACH_EXPRESSION_END, lf_point(source, at(source, ' as'))),
		null
	);
	// inside a token
	strictEqual(classify(source, root, EACH_EXPRESSION_END, lf_point(source, s + 3)), null);
	// the start side is never stale
	strictEqual(
		classify(source, root, 'fragment.nodes[0].expression.loc.start.column', { column: 1 }),
		null
	);
	// an expression that is no `{#each}`'s
	strictEqual(
		classify(source, root, 'fragment.nodes[1].expression.loc.end.column', { column: s + 18 }),
		null
	);
});

// --- languages ---------------------------------------------------------------------

Deno.test('no row applies to a TypeScript or CSS document', () => {
	const ts = 'let a = 1;\u2028let b = 2;';
	const program = node(ts, 'Program', 0, ts.length, {
		body: [node(ts, 'VariableDeclaration', 0, 10), node(ts, 'VariableDeclaration', 11, 21)]
	});
	strictEqual(
		classify(
			ts,
			program,
			'body[1].loc.start.line',
			{ line: 2, column: 0 },
			{ language: 'typescript' }
		),
		null
	);
	// the destructure shift, exactly as the Svelte row reads it, under the other languages
	const [source, root] = each_with_context('\n', '{ a, b }', 'ObjectPattern');
	const column = lf_point(source, at(source, '{ a')).column;
	for (const language of ['typescript', 'css'] as const) {
		strictEqual(
			classify(source, root, CONTEXT_START_COLUMN, { column: column + 1 }, { language }),
			null
		);
	}
	const css = 'p {\n\tcolor: red;\r}';
	const sheet = node(css, 'StyleSheetFile', 0, css.length, {
		children: [node(css, 'Rule', 0, css.length)]
	});
	strictEqual(
		classify(css, sheet, 'children[0].loc.end.line', { line: 3, column: 0 }, { language: 'css' }),
		null
	);
});

// --- the superset pin ---------------------------------------------------------------

Deno.test('superset: TypeScript has none; Svelte and CSS pin their reader-built kinds', () => {
	strictEqual(superset_kinds_of('typescript'), null);
	const svelte = superset_kinds_of('svelte')!;
	const css = superset_kinds_of('css')!;
	strictEqual(svelte.has('RegularElement'), true);
	strictEqual(svelte.has('Rule'), true);
	strictEqual(css.has('Rule'), true);
	// acorn's own node types are never pinned plainly
	strictEqual(svelte.has('Identifier'), false);
	strictEqual(svelte.has('Program'), false);
	strictEqual(css.has('RegularElement'), false);
});

Deno.test('superset_key: a plain type, a shape, an attached comment, an untyped span', () => {
	const svelte = superset_kinds_of('svelte')!;
	const span = { start: 0, end: 1 };
	strictEqual(
		superset_key({ ...span, type: 'RegularElement' }, 'fragment.nodes[0]', null, svelte, ''),
		'RegularElement'
	);
	// a directive's shorthand identifier is Svelte-built; an island's is acorn's
	const shorthand = superset_key(
		{ ...span, type: 'Identifier' },
		'fragment.nodes[0].attributes[2].expression',
		{ type: 'BindDirective', path: 'fragment.nodes[0].attributes[2]' },
		svelte,
		''
	);
	strictEqual(shorthand, 'BindDirective.expression:Identifier');
	strictEqual(svelte.has(shorthand), true);
	const island_identifier = superset_key(
		{ ...span, type: 'Identifier' },
		'fragment.nodes[1].expression',
		{ type: 'ExpressionTag', path: 'fragment.nodes[1]' },
		svelte,
		''
	);
	strictEqual(island_identifier, 'ExpressionTag.expression:Identifier');
	strictEqual(svelte.has(island_identifier), false);
	// a script node keys by its whole path under the `<script>`, which nothing pins
	const script_node = superset_key(
		{ ...span, type: 'Identifier' },
		'instance.content.body[0].expression',
		{ type: 'Script', path: 'instance' },
		svelte,
		''
	);
	strictEqual(script_node, 'Script.content.body[].expression:Identifier');
	strictEqual(svelte.has(script_node), false);
	// an attached comment keys by its list — and a block comment is no CSS `Block`
	strictEqual(
		superset_key(
			{ ...span, type: 'Block', value: ' c ' },
			'instance.content.body[0].leadingComments[0]',
			{ type: 'Script', path: 'instance' },
			svelte,
			''
		),
		'leadingComments[]:Block'
	);
	const root_comment = superset_key(
		{ ...span, type: 'Line', value: ' c' },
		'comments[0]',
		{ type: 'Root', path: '' },
		svelte,
		''
	);
	strictEqual(root_comment, 'comments[]:Line');
	strictEqual(svelte.has(root_comment), false);
	strictEqual(
		superset_key(
			{ ...span, styles: '' },
			'css.content',
			{ type: 'StyleSheet', path: 'css' },
			svelte,
			''
		),
		'StyleSheet.content:'
	);
});

Deno.test('superset_key: a braced spelling of a pinned shape is acorn-parsed, never pinned', () => {
	const svelte = superset_kinds_of('svelte')!;
	/** The key of the last `needle` in `source`, the directive or element `anchor`'s `slot`. */
	const key = (source: string, needle: string, type: string, anchor: string, slot: string) => {
		const start = source.lastIndexOf(needle);
		return superset_key(
			{ type, start, end: start + needle.length },
			`fragment.nodes[0].attributes[0].${slot}`,
			{ type: anchor, path: 'fragment.nodes[0].attributes[0]' },
			svelte,
			source
		);
	};
	const cases = [
		// [Svelte-built spelling, acorn spelling, needle, type, anchor, slot]
		[
			'<input bind:value />',
			'<input bind:value={ value } />',
			'value',
			'Identifier',
			'BindDirective',
			'expression'
		],
		[
			'<div class:on></div>',
			'<div class:on={on}></div>',
			'on',
			'Identifier',
			'ClassDirective',
			'expression'
		],
		[
			'<svelte:element this="div" />',
			'<svelte:element this={"div"} />',
			'"div"',
			'Literal',
			'SvelteElement',
			'tag'
		]
	] as const;
	for (const [bare, braced, needle, type, anchor, slot] of cases) {
		const built = key(bare, needle, type, anchor, slot);
		strictEqual(built, `${anchor}.${slot}:${type}`);
		strictEqual(svelte.has(built), true, built);
		const parsed = key(braced, needle, type, anchor, slot);
		strictEqual(parsed, `${anchor}.${slot}:{${type}}`);
		strictEqual(svelte.has(parsed), false, parsed);
	}
	// a brace outside those slots is no island: `{ @const}`'s declaration is Svelte-built
	const spaced_const = '{ @const a = b}';
	const declaration = superset_key(
		{ type: 'VariableDeclaration', start: 2, end: spaced_const.length - 1 },
		'fragment.nodes[0].declaration',
		{ type: 'ConstTag', path: 'fragment.nodes[0]' },
		svelte,
		spaced_const
	);
	strictEqual(declaration, 'ConstTag.declaration:VariableDeclaration');
	strictEqual(svelte.has(declaration), true);
	// an attached comment behind a `{` still keys by its list
	const island = '{/* c */ x}';
	strictEqual(
		superset_key(
			{ type: 'Block', value: ' c ', start: 1, end: 8 },
			'fragment.nodes[0].expression.leadingComments[0]',
			{ type: 'ExpressionTag', path: 'fragment.nodes[0]' },
			svelte,
			island
		),
		'leadingComments[]:Block'
	);
});

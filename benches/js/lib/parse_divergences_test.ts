/**
 * Tests for the documented parse divergences (`DOCUMENTED_MATCHERS`): each tested matcher
 * gets the difference it exists for, which it must claim, and a near miss — the same entry
 * one step off (a direction, a missing precondition, a neighboring path) — which it must
 * not, since a matcher that admits a near miss is one that would excuse a tsv bug landing
 * there. And every matcher's `conformance_section` must name a section the catalog holds.
 *
 * Hand-written canonical trees holding only the fields the matchers read, on purpose: a
 * test that parsed through the oracle would need the sidecar and stop gating in
 * `deno task check` (see `test:deno`). What the real oracle writes is graded by
 * `corpus:compare:parse`.
 */

import { ok, strictEqual } from 'node:assert';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import type { DiffEntry, MatchContext } from './parse_diff.ts';
import { get_at_path } from './diff_path.ts';
import { DOCUMENTED_MATCHERS, type DocumentedMatcher } from './parse_divergences.ts';
import type { Language } from './types.ts';

type Entry = Omit<DiffEntry, 'documented' | 'signature'>;

function matcher(name: string): DocumentedMatcher {
	const found = DOCUMENTED_MATCHERS.find((m) => m.name === name);
	ok(found, `no matcher named ${name}`);
	return found;
}

/** Whether matcher `name` claims `entry`, its canonical parent resolved off `canonical_root`. */
function claims(
	name: string,
	entry: Entry,
	canonical_root: unknown,
	language: Language = 'typescript',
	source = ''
): boolean {
	const ctx: MatchContext = { source, canonical_root, language };
	const parent_path = entry.path.replace(/(?:\.[^.[]+|\[\d+\])$/, '');
	return matcher(name).matches(entry, get_at_path(canonical_root, parent_path), ctx);
}

Deno.test('matcher names are unique', () => {
	const names = DOCUMENTED_MATCHERS.map((m) => m.name);
	strictEqual(new Set(names).size, names.length);
});

Deno.test('lone_surrogate_value: U+FFFD for a lone surrogate, and only that', () => {
	const entry = (ours: string, canonical: string): Entry => ({
		kind: 'value_mismatch',
		path: 'body[0].expression.value',
		ours,
		canonical
	});
	strictEqual(claims('lone_surrogate_value', entry('a\u{FFFD}', 'a\uD800'), {}), true);
	strictEqual(claims('lone_surrogate_value', entry('\u{FFFD}', '\uDC00'), {}), true);
	// a paired surrogate is no lone one
	strictEqual(claims('lone_surrogate_value', entry('\u{FFFD}', '\u{1F600}'), {}), false);
	// the replacement must yield OUR value
	strictEqual(claims('lone_surrogate_value', entry('b\u{FFFD}', 'a\uD800'), {}), false);
	// no surrogate at all
	strictEqual(claims('lone_surrogate_value', entry('a', 'b'), {}), false);
});

Deno.test('rest_param_type_end: the annotation end against the binding end, nothing else', () => {
	// `(...args: Array<any>) => 1`, offsets as acorn-typescript writes them
	const rest = (fields: Record<string, unknown>) => ({
		params: [
			{ type: 'Identifier', typeAnnotation: { end: 9 } },
			{ type: 'RestElement', argument: { end: 18 }, typeAnnotation: { end: 30 }, ...fields }
		]
	});
	const end: Entry = { kind: 'value_mismatch', path: 'params[1].end', ours: 30, canonical: 18 };
	strictEqual(claims('rest_param_type_end', end, rest({})), true);
	// ours must be the annotation's end, canonical's the binding's
	strictEqual(claims('rest_param_type_end', { ...end, ours: 29 }, rest({})), false);
	strictEqual(claims('rest_param_type_end', { ...end, canonical: 17 }, rest({})), false);
	// untyped rest: acorn-typescript and tsv agree, so a difference there is a bug
	strictEqual(claims('rest_param_type_end', end, rest({ typeAnnotation: undefined })), false);
	// a typed Identifier param ends after its annotation on both sides
	strictEqual(claims('rest_param_type_end', { ...end, path: 'params[0].end' }, rest({})), false);
	// the start is not what diverges
	strictEqual(claims('rest_param_type_end', { ...end, path: 'params[1].start' }, rest({})), false);
});

Deno.test('comment_dedup: the canonical extras must BE duplicates of what tsv lists', () => {
	const span = (start: number, end: number, value = '') => ({ type: 'Block', start, end, value });
	const attached = (comments: unknown[]) => ({ body: [{ leadingComments: comments }] });
	const length = (ours: number, canonical: number): Entry => ({
		kind: 'length_mismatch',
		path: 'body[0].leadingComments',
		ours,
		canonical
	});
	strictEqual(claims('comment_dedup', length(1, 2), attached([span(0, 5), span(0, 5)])), true);
	// tsv DROPPED a comment: the canonical extra is a distinct comment, not a copy
	strictEqual(claims('comment_dedup', length(1, 2), attached([span(0, 5), span(6, 9)])), false);
	// a duplicate exists, but tsv lists fewer than the distinct comments
	strictEqual(
		claims('comment_dedup', length(1, 3), attached([span(0, 5), span(0, 5), span(6, 9)])),
		false
	);
	// two span-less copies (Svelte's HTML-comment copies) are two comments, not one
	const html = (value: string) => ({ type: 'Line', value });
	strictEqual(claims('comment_dedup', length(1, 2), attached([html(' a '), html(' a ')])), false);
	// tsv emitting MORE comments is not a duplication on the oracle's side
	strictEqual(claims('comment_dedup', length(2, 1), attached([span(0, 5)])), false);

	// a root `comments[i]` drift: ours is the i-th DISTINCT canonical comment
	const drift = (index: number, ours: number, canonical: number): Entry => ({
		kind: 'value_mismatch',
		path: `comments[${index}].start`,
		ours,
		canonical
	});
	const duplicated = { comments: [span(4, 6), span(4, 6), span(9, 11)] };
	strictEqual(claims('comment_dedup', drift(1, 9, 4), duplicated), true);
	// a value no distinct comment holds at that index
	strictEqual(claims('comment_dedup', drift(1, 12, 4), duplicated), false);
	// no duplicate anywhere: a drift is a real offset bug
	strictEqual(
		claims('comment_dedup', drift(1, 9, 4), { comments: [span(4, 6), span(9, 11)] }),
		false
	);
	// a drift AHEAD of the duplicate has no shift to explain it
	const late = { comments: [span(1, 2), span(4, 6), span(4, 6)] };
	strictEqual(claims('comment_dedup', drift(0, 0, 1), late), false);

	// an attached copy whose value disagrees with its twin: ours must be one twin's
	const twins = attached([span(0, 9, ' a\nb '), span(0, 9, ' a\n\tb ')]);
	const value = (ours: string): Entry => ({
		kind: 'value_mismatch',
		path: 'body[0].leadingComments[0].value',
		ours,
		canonical: ' a\nb '
	});
	strictEqual(claims('comment_dedup', value(' a\n\tb '), twins), true);
	strictEqual(claims('comment_dedup', value(' a\n  b '), twins), false);
});

Deno.test('svelte_template_paren_comment: a leading comment ahead of a stripped paren', () => {
	const source = '{/* c */ (a) + b}';
	const comment = { type: 'Block', start: 1, end: 8 };
	const root = (start: number, comments: unknown[] = [comment]) => ({
		fragment: { nodes: [{ expression: { left: { type: 'Identifier', start } } }] },
		comments
	});
	const at_a = source.indexOf('a');
	const entry = (kind: Entry['kind'], path: string, ours: unknown = [comment]): Entry => ({
		kind,
		path,
		ours,
		canonical: undefined
	});
	const path = 'fragment.nodes[0].expression.left.leadingComments';
	const paren = (e: Entry, s = source, r: unknown = root(at_a), language: Language = 'svelte') =>
		claims('svelte_template_paren_comment', e, r, language, s);
	strictEqual(paren(entry('missing_canonical', path)), true);
	// no paren in front of the node: nothing was stripped
	const bare = '{/* c */ a + b}';
	strictEqual(paren(entry('missing_canonical', path), bare, root(bare.indexOf('a'))), false);
	// the comment sits INSIDE the paren: both parsers attach it
	const inside = '{(/* c */ a) + b}';
	const inner = [{ type: 'Block', start: 2, end: 9 }];
	strictEqual(
		paren(entry('missing_canonical', path, inner), inside, root(inside.indexOf('a'))),
		false
	);
	// the trailing side of the wrapper attaches in both parsers
	strictEqual(paren(entry('missing_canonical', path.replace('leading', 'trailing'))), false);
	// a `<script>` parse sets no preserveParens, so the same attachment there is a bug
	const script = 'instance.content.body[0].expression.left.leadingComments';
	strictEqual(paren(entry('missing_canonical', script)), false);
	// the direction: a comment tsv LACKS is a drop
	strictEqual(paren(entry('missing_ours', path)), false);
	// a CALL's paren: the comment leads the callee, and a copy on the argument is a bug
	const call = '{/* c */ f(a)}';
	const argument = 'fragment.nodes[0].expression.arguments[0].leadingComments';
	const call_root = {
		fragment: { nodes: [{ expression: { arguments: [{ start: call.indexOf('a)') }] } }] }
	};
	strictEqual(paren(entry('missing_canonical', argument), call, call_root), false);
	// canonical attached the comment elsewhere (here the callee's trailing run, ahead of
	// the call's paren): no wrapper discarded it, so tsv's copy is an attachment difference
	const glued = '{f /* c */ (a)}';
	const glued_comment = { type: 'Block', start: 3, end: 10 };
	const trailing_callee = (trailing: unknown[]) => ({
		comments: [glued_comment],
		fragment: {
			nodes: [
				{
					expression: {
						callee: { trailingComments: trailing },
						arguments: [{ start: glued.indexOf('a)') }]
					}
				}
			]
		}
	});
	const on_argument = entry('missing_canonical', argument, [glued_comment]);
	strictEqual(paren(on_argument, glued, trailing_callee([])), true);
	strictEqual(paren(on_argument, glued, trailing_callee([{ ...glued_comment }])), false);
	// a leading BOM: Svelte's offsets index the BOM-less text
	strictEqual(paren(entry('missing_canonical', path), '﻿' + source), true);
	// tsv's list must be exactly the run ahead of the `(`, in order, each once
	const two = '{/* c */ /* d */ (a) + b}';
	const c = { type: 'Block', start: 1, end: 8 };
	const d = { type: 'Block', start: 9, end: 16 };
	const two_root = root(two.indexOf('a'), [c, d]);
	strictEqual(paren(entry('missing_canonical', path, [c, d]), two, two_root), true);
	strictEqual(paren(entry('missing_canonical', path, [c, c]), two, two_root), false);
	strictEqual(paren(entry('missing_canonical', path, [d]), two, two_root), false);
	strictEqual(paren(entry('missing_canonical', path, [d, c]), two, two_root), false);
});

Deno.test('manufactured_line_comment_dedent: a dedent measured on a manufactured line', () => {
	// `read_script` hands acorn `<script>` blanked to eight spaces, so Svelte strips eight
	// spaces the author typed where the document's line opens with no run at all
	const source = '<script>/*\n        c */</script>';
	const comment = { type: 'Block', start: 8, end: source.indexOf('*/') + 2 };
	const root = (start = 0) => ({
		instance: {
			content: {
				trailingComments: [{ ...comment, start: comment.start + start, end: comment.end + start }]
			}
		}
	});
	const entry = (ours: string, canonical: string): Entry => ({
		kind: 'value_mismatch',
		path: 'instance.content.trailingComments[0].value',
		ours,
		canonical
	});
	const dedent = (e: Entry, s = source, r: unknown = root()) =>
		claims('manufactured_line_comment_dedent', e, r, 'svelte', s);
	strictEqual(dedent(entry('\n        c ', '\nc ')), true);
	// canonical undedented: the run Svelte measured matched no line
	strictEqual(dedent(entry('\n        c ', '\n        c ')), true);
	// ours must be the document line's dedent (none, here)
	strictEqual(dedent(entry('\nc ', '\n        c ')), false);
	// canonical must be a uniform dedent of the comment's own text
	strictEqual(dedent(entry('\n        c ', '\nx ')), false);
	// a comment on a line the document opens is measured alike by both parsers
	const below = '<script>\n/*\n        c */</script>';
	strictEqual(dedent(entry('\n        c ', '\nc '), below, root(1)), false);
	// a leading BOM: Svelte's offsets index the BOM-less text
	strictEqual(dedent(entry('\n        c ', '\nc '), '﻿' + source), true);
});

Deno.test('svelte_instance_comment_duplication: a module-region prefix and nothing else', () => {
	// the instance script starts at 50; the module-region comment at 10 is copied ahead of
	// the instance's own comment at 60
	const comment = (start: number, value: string) => ({
		type: 'Line',
		value,
		start,
		end: start + value.length + 2
	});
	const module_comment = comment(10, ' m');
	const own = comment(60, ' own');
	const root = (comments: unknown[]) => ({
		instance: { content: { start: 50, body: [{ leadingComments: comments }] } }
	});
	const path = 'instance.content.body[0].leadingComments';
	const dup = (e: Entry, r: unknown) =>
		claims('svelte_instance_comment_duplication', e, r, 'svelte');
	const length = (ours: number): Entry => ({
		kind: 'length_mismatch',
		path,
		ours,
		canonical: 2
	});
	// tsv lists the instance's own comment: longer by exactly the prefix
	strictEqual(dup(length(1), root([module_comment, own])), true);
	// tsv lists none, though the instance HAS its own: a drop
	strictEqual(dup(length(0), root([module_comment, own])), false);
	// no module-region prefix at all
	strictEqual(dup(length(1), root([own, comment(70, ' x')])), false);
	// tsv has no array: every canonical entry must be a module-region copy
	const missing: Entry = { kind: 'missing_ours', path, ours: undefined, canonical: [] };
	strictEqual(dup(missing, root([module_comment])), true);
	strictEqual(dup(missing, root([module_comment, own])), false);
	// each field reads one whole prefix later
	const field = (ours: unknown): Entry => ({
		kind: 'value_mismatch',
		path: `${path}[0].value`,
		ours,
		canonical: ' m'
	});
	strictEqual(dup(field(' own'), root([module_comment, own])), true);
	strictEqual(dup(field(' other'), root([module_comment, own])), false);
	// a leading HTML comment's span-less copy is no module-region comment of this one's
	const html = { type: 'Line', value: ' @component docs ' };
	const program: Entry = {
		kind: 'missing_ours',
		path: 'instance.content.leadingComments',
		ours: undefined,
		canonical: [html]
	};
	strictEqual(
		dup(program, {
			module: { content: { start: 20, leadingComments: [{ ...html }] } },
			instance: { content: { start: 50, leadingComments: [html] } }
		}),
		false
	);
	// the same shape outside the instance script is not this divergence
	strictEqual(
		dup(
			{ ...length(1), path: 'module.content.body[0].leadingComments' },
			root([module_comment, own])
		),
		false
	);
});

/**
 * A Svelte document's top-level shape as the oracle writes it: its fragment nodes (a lifted
 * tag is never one, so a hole is left where it sits) and each lifted root's tag span, read
 * off `source` — a `<script module>`, a `<script>`, a `<style>`, each as its own tag pair.
 */
function lifted_document(source: string): Record<string, unknown> {
	const root: Record<string, unknown> = {};
	const holes: [number, number][] = [];
	for (const m of source.matchAll(/<(script|style)\b([^>]*)>[\s\S]*?<\/\1>/g)) {
		const key = m[1] === 'style' ? 'css' : m[2]!.includes('module') ? 'module' : 'instance';
		const start = m.index!;
		root[key] = { start, end: start + m[0].length, content: {} };
		holes.push([start, start + m[0].length]);
	}
	const nodes: unknown[] = [];
	let at = 0;
	for (const [start, end] of [...holes, [source.length, source.length]]) {
		const between = source.slice(at, start);
		for (const m of between.matchAll(/<!--([\s\S]*?)-->|[^<]+/g)) {
			const node_start = at + m.index!;
			const node_end = node_start + m[0].length;
			nodes.push(
				m[1] !== undefined
					? { type: 'Comment', start: node_start, end: node_end, data: m[1] }
					: { type: 'Text', start: node_start, end: node_end, raw: m[0], data: m[0] }
			);
		}
		at = end;
	}
	root.fragment = { type: 'Fragment', nodes };
	return root;
}

Deno.test('svelte_lifted_root_comment_duplication: a copy only past another lifted root', () => {
	const copy = (value: string) => ({ type: 'Line', value });
	const script_copy = (key: 'instance' | 'module', value = ' c '): Entry => ({
		kind: 'missing_ours',
		path: `${key}.content.leadingComments`,
		ours: undefined,
		canonical: [copy(value)]
	});
	const lifted = (e: Entry, source: string) =>
		claims('svelte_lifted_root_comment_duplication', e, lifted_document(source), 'svelte');
	const module_instance = '<!-- c -->\n<script module>\n</script>\n<script>\n</script>\n';
	const instance_module = '<!-- c -->\n<script>\n</script>\n\n<script module>\n</script>\n';
	const style_script = '<!-- c -->\n<style>\n</style>\n\n<script>\n</script>\n';
	const script_style = '<!-- c -->\n<script>\n</script>\n\n<style>\n</style>\n';
	// the copy on each later root, in every arrangement
	strictEqual(lifted(script_copy('instance'), module_instance), true);
	strictEqual(lifted(script_copy('module'), instance_module), true);
	strictEqual(lifted(script_copy('instance'), style_script), true);
	const comment = { type: 'Comment', start: 0, end: 10, data: ' c ' };
	const css_copy = (canonical: unknown): Entry => ({
		kind: 'type_mismatch',
		path: 'css.content.comment',
		ours: null,
		canonical
	});
	strictEqual(lifted(css_copy(comment), script_style), true);
	// the NEAREST root's copy is tsv's to make: never excused
	strictEqual(lifted(script_copy('module'), module_instance), false);
	strictEqual(lifted(script_copy('instance'), instance_module), false);
	strictEqual(lifted(css_copy(comment), style_script), false);
	// the root's OWN comment, though an earlier one past a lifted root has the same text:
	// dropping it is a bug the earlier comment must not excuse
	const same_text = '<!-- x -->\n<script module>\n</script>\n<!-- x -->\n<script>\n</script>\n';
	strictEqual(lifted(script_copy('instance', ' x '), same_text), false);
	// the copy must be the comment's own text
	strictEqual(lifted(script_copy('instance', ' d '), module_instance), false);
	// non-whitespace text between: Svelte's walk stops there, so a copy is no such dup
	const worded = '<!-- c -->\n<script module>\n</script>\nhi\n<script>\n</script>\n';
	strictEqual(lifted(script_copy('instance'), worded), false);
	// the tag glued to an earlier root: Svelte's walk needs a node ending AT the tag
	const glued = '<!-- c -->\n<script module>\n</script><script>\n</script>\n';
	strictEqual(lifted(script_copy('instance'), glued), false);
	// the directions: tsv carrying a copy canonical lacks, or a `<style>` copy tsv has
	strictEqual(
		lifted({ ...script_copy('instance'), kind: 'missing_canonical' }, module_instance),
		false
	);
	strictEqual(
		lifted({ ...css_copy(comment), ours: comment, canonical: null }, script_style),
		false
	);
	// a different comment than the walk reaches
	strictEqual(lifted(css_copy({ ...comment, data: ' d ' }), script_style), false);
});

Deno.test(
	'extends_instantiation_linebreak: the instantiation unwrapped, across a line break',
	() => {
		const broken = 'class A extends B<T>\n\timplements I {}';
		const one_line = 'class A extends B<T> implements I {}';
		const expression = { type: 'Identifier', start: 16, end: 17, name: 'B' };
		const type_arguments = {
			type: 'TSTypeParameterInstantiation',
			start: 17,
			end: 20,
			params: [{ type: 'TSTypeReference', start: 18, end: 19 }]
		};
		const root = {
			body: [
				{
					type: 'ClassDeclaration',
					superClass: {
						type: 'TSInstantiationExpression',
						start: 16,
						end: 20,
						expression,
						typeArguments: type_arguments
					}
				}
			]
		};
		const entry = (
			kind: Entry['kind'],
			path: string,
			ours: unknown,
			canonical?: unknown
		): Entry => ({
			kind,
			path,
			ours,
			canonical
		});
		const shape = (e: Entry, source = broken) =>
			claims('extends_instantiation_linebreak', e, root, 'typescript', source);
		const type = entry('value_mismatch', 'body[0].superClass.type', 'Identifier');
		strictEqual(shape(type), true);
		strictEqual(shape(entry('value_mismatch', 'body[0].superClass.end', 17, 20)), true);
		strictEqual(shape(entry('missing_canonical', 'body[0].superClass.name', 'B')), true);
		strictEqual(shape(entry('missing_ours', 'body[0].superClass.expression', undefined, {})), true);
		// key order is no difference
		const reordered = {
			params: type_arguments.params,
			end: 20,
			start: 17,
			type: type_arguments.type
		};
		strictEqual(shape(entry('missing_canonical', 'body[0].superTypeParameters', reordered)), true);
		// the same line: acorn-typescript emits tsv's shape, so a difference is a bug
		strictEqual(shape(type, one_line), false);
		// a line break inside a comment is still a preceding line break
		strictEqual(shape(type, 'class A extends B<T> /*\n*/ implements I {}'), true);
		// a wrong superclass, a wrong end, wrong type arguments
		strictEqual(shape(entry('missing_canonical', 'body[0].superClass.name', 'C')), false);
		strictEqual(shape(entry('value_mismatch', 'body[0].superClass.end', 18, 20)), false);
		const wrong = { ...type_arguments, params: [] };
		strictEqual(shape(entry('missing_canonical', 'body[0].superTypeParameters', wrong)), false);
	}
);

Deno.test('decorator_paren_subscript_start: ours at the `(`, canonical just inside it', () => {
	const source = '@(f)() class A {}';
	const root = {
		body: [{ decorators: [{ expression: { type: 'CallExpression', start: 2, end: 8 } }] }]
	};
	const entry = (
		ours: number,
		canonical: number,
		kind: Entry['kind'] = 'value_mismatch'
	): Entry => ({
		kind,
		path: 'body[0].decorators[0].expression.start',
		ours,
		canonical
	});
	const start = (e: Entry, s = source) =>
		claims('decorator_paren_subscript_start', e, root, 'typescript', s);
	strictEqual(start(entry(1, 2)), true);
	// further opening parens between are the same paren run
	strictEqual(start(entry(1, 3), '@((f))() class A {}'), true);
	// ours is not a `(`
	strictEqual(start(entry(0, 2)), false);
	// canonical is past the inner expression's first token
	strictEqual(start(entry(1, 3)), false);
	// the wrong direction
	strictEqual(start(entry(2, 1)), false);
	// ours at a paren NESTED in the decorator's: tsv would start one paren late
	strictEqual(start(entry(2, 3), '@((f))() class A {}'), false);
	// the expression INSIDE the parens shares canonical's start but ends at their `)`:
	// tsv starting it at the `(` is a bug, not this divergence
	const member = '@(a.b)() p';
	const member_root = {
		body: [
			{
				decorators: [
					{
						expression: {
							type: 'CallExpression',
							start: 2,
							end: 8,
							callee: { type: 'MemberExpression', start: 2, end: 5 }
						}
					}
				]
			}
		]
	};
	const callee = (ours: number, canonical: number): Entry => ({
		kind: 'value_mismatch',
		path: 'body[0].decorators[0].expression.callee.start',
		ours,
		canonical
	});
	strictEqual(
		claims('decorator_paren_subscript_start', callee(1, 2), member_root, 'typescript', member),
		false
	);
});

Deno.test(
	'css_declaration_tokenization: the garbage declaration and the comment tail it shifts',
	() => {
		const garbage = {
			type: 'Declaration',
			start: 20,
			end: 40,
			property: 'color/*',
			value: 'c */: red'
		};
		const sheet = (declaration: Record<string, unknown>) => ({
			children: [{ type: 'Rule', block: { children: [declaration] } }],
			comments: [
				{ type: 'Comment', start: 1, end: 8 },
				{ type: 'Comment', start: 50, end: 57 }
			]
		});
		const root = sheet(garbage);
		const clean = sheet({ ...garbage, property: 'color' });
		const css = (e: Entry, r: unknown = root, language: Language = 'css') =>
			claims('css_declaration_tokenization', e, r, language);
		// (1) the declaration's own fields
		const property: Entry = {
			kind: 'value_mismatch',
			path: 'children[0].block.children[0].property',
			ours: 'color',
			canonical: 'color/*'
		};
		strictEqual(css(property), true);
		strictEqual(css(property, clean), false);
		// (2) the comment array: longer on OUR side, from the first comment past the garbage on
		const length = (ours: number): Entry => ({
			kind: 'length_mismatch',
			path: 'comments',
			ours,
			canonical: 2
		});
		strictEqual(css(length(3)), true);
		strictEqual(css(length(1)), false);
		strictEqual(css(length(3), clean), false);
		const shifted = (index: number): Entry => ({
			kind: 'value_mismatch',
			path: `comments[${index}].start`,
			ours: 25,
			canonical: 50
		});
		strictEqual(css(shifted(1)), true);
		// a comment AHEAD of the garbage pairs with its own
		strictEqual(css(shifted(0)), false);
		// tsv emitting no comments array at all
		strictEqual(
			css({ kind: 'missing_ours', path: 'comments', ours: undefined, canonical: [] }),
			false
		);
		// a Svelte document's stylesheet
		strictEqual(
			css({ ...shifted(1), path: 'css.comments[1].start' }, { css: root }, 'svelte'),
			true
		);
		// a Svelte root's `comments` is its JS comment list, which the garbage cannot shift
		const svelte_root = { css: root, comments: root.comments };
		strictEqual(css(shifted(1), svelte_root, 'svelte'), false);
		strictEqual(css(length(3), svelte_root, 'svelte'), false);
	}
);

Deno.test('type_assertion_paren_arrow: a doubled paren after the type parameters', () => {
	const source = 'x = <any>(() => {});';
	const root = (end: number) => ({
		body: [
			{
				expression: {
					right: { type: 'ArrowFunctionExpression', typeParameters: { end } }
				}
			}
		]
	});
	const entry: Entry = {
		kind: 'value_mismatch',
		path: 'body[0].expression.right.type',
		ours: 'TSTypeAssertion',
		canonical: 'ArrowFunctionExpression'
	};
	const assertion = (s: string, end: number, language: Language = 'typescript') =>
		claims('type_assertion_paren_arrow', entry, root(end), language, s);
	strictEqual(assertion(source, source.indexOf('>') + 1), true);
	// a real generic arrow's parameter list opens with one paren
	const generic = 'x = <T>(a) => a;';
	strictEqual(assertion(generic, generic.indexOf('>') + 1), false);
	// a leading BOM: in a Svelte document the offsets index the BOM-less text
	const svelte = '<script lang="ts">x = <any>(() => {});</script>';
	strictEqual(assertion('﻿' + svelte, svelte.indexOf('>(') + 1, 'svelte'), true);
});

Deno.test('static_member_ladder: an ASI-split `static`, from the ladder on', () => {
	// `class C {⏎static;⏎static⏎static⏎a() {}⏎}` as acorn-typescript reads it
	const source = 'class C {\n\tstatic;\n\tstatic\n\tstatic\n\ta() {}\n}';
	const field = (start: number, end: number) => ({
		type: 'PropertyDefinition',
		start,
		end,
		static: false,
		computed: false,
		value: null,
		key: { type: 'Identifier', start, end: start + 6, name: 'static' }
	});
	const method = { type: 'MethodDefinition', start: 36, end: 42 };
	const root = (members: unknown[]) => ({
		body: [{ type: 'ClassDeclaration', body: { type: 'ClassBody', body: members } }]
	});
	const ladder = root([field(11, 18), field(20, 26), field(28, 34), method]);
	const entry = (path: string, kind: Entry['kind'] = 'value_mismatch'): Entry => ({
		kind,
		path,
		ours: 1,
		canonical: 2
	});
	const claim = (e: Entry, r: unknown = ladder) =>
		claims('static_member_ladder', e, r, 'typescript', source);
	strictEqual(claim(entry('body[0].body.body[1].start')), true);
	strictEqual(claim(entry('body[0].body.body[3].key.name')), true);
	strictEqual(claim(entry('body[0].body.body', 'length_mismatch')), true);
	// the written `static;` ahead of the ladder pairs with its own
	strictEqual(claim(entry('body[0].body.body[0].end')), false);
	// a written `static;` alone is no ladder
	const written = root([field(11, 18), { ...method, start: 20 }]);
	strictEqual(claim(entry('body[0].body.body[1].start'), written), false);
	strictEqual(claim(entry('body[0].body.body', 'length_mismatch'), written), false);
	// a written `static static⏎x() {}`: a STATIC field of the split's shape, no ladder
	const doubled_source = 'class C {\n\tstatic static\n\tx() {}\n}';
	const doubled = root([
		{
			...field(11, 24),
			static: true,
			key: { type: 'Identifier', start: 18, end: 24, name: 'static' }
		},
		{ ...method, start: 26, end: 32 }
	]);
	strictEqual(
		claims(
			'static_member_ladder',
			entry('body[0].body.body[1].start'),
			doubled,
			'typescript',
			doubled_source
		),
		false
	);
});

Deno.test("nth_of_structure: anchored on Svelte's `of` word in an Nth value", () => {
	const root = (value: string) => ({
		children: [{ prelude: { args: { children: [{ type: 'Nth', value }] } } }]
	});
	const entry: Entry = {
		kind: 'value_mismatch',
		path: 'children[0].prelude.args.children[0].value',
		ours: '2n',
		canonical: '2n of '
	};
	strictEqual(claims('nth_of_structure', entry, root('2n of '), 'css'), true);
	strictEqual(claims('nth_of_structure', entry, root('2n of'), 'css'), true);
	// a plain Nth carries no tell
	strictEqual(claims('nth_of_structure', entry, root('2n'), 'css'), false);
	// a content field inside `S` is not reshape-shaped
	const name: Entry = { ...entry, path: 'children[0].prelude.args.children[1].name' };
	strictEqual(claims('nth_of_structure', name, root('2n of '), 'css'), false);
});

/**
 * Every `#` heading of the catalog, each with the text up to the next heading at its level
 * or above — the section a `conformance_section` entry has to sit in.
 */
function catalog_sections(markdown: string): Map<string, string> {
	const lines = markdown.split('\n');
	const headings = lines.flatMap((line, i) => {
		const m = /^(#+)\s+(.*?)\s*$/.exec(line);
		return m ? [{ i, level: m[1]!.length, title: m[2]! }] : [];
	});
	return new Map(
		headings.map(({ i, level, title }) => {
			const end = headings.find((h) => h.i > i && h.level <= level)?.i ?? lines.length;
			return [title, lines.slice(i + 1, end).join('\n')];
		})
	);
}

Deno.test('every conformance_section names a docs/conformance_svelte.md section', () => {
	const doc = readFileSync(
		fileURLToPath(new URL('../../../docs/conformance_svelte.md', import.meta.url)),
		'utf8'
	);
	const sections = catalog_sections(doc);
	for (const m of DOCUMENTED_MATCHERS) {
		// `Heading — Entry`: the heading is the section, the entry a named item inside it
		const [heading, entry] = m.conformance_section.split(' — ', 2) as [string, string?];
		const body = sections.get(heading);
		ok(body !== undefined, `${m.name}: no heading "${heading}" in conformance_svelte.md`);
		if (entry !== undefined) {
			ok(body.includes(entry), `${m.name}: "${entry}" is not under "${heading}"`);
		}
	}
});

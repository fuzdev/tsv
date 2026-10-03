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

Deno.test('rest_param_type_end: the end of a typed RestElement only', () => {
	const root = (rest: Record<string, unknown>) => ({
		params: [{ type: 'Identifier', typeAnnotation: {} }, rest]
	});
	const end: Entry = { kind: 'value_mismatch', path: 'params[1].end', ours: 20, canonical: 12 };
	const typed = root({ type: 'RestElement', typeAnnotation: {} });
	strictEqual(claims('rest_param_type_end', end, typed), true);
	// untyped rest: acorn-typescript and tsv agree, so a difference there is a bug
	strictEqual(claims('rest_param_type_end', end, root({ type: 'RestElement' })), false);
	// a typed Identifier param ends after its annotation on both sides
	strictEqual(claims('rest_param_type_end', { ...end, path: 'params[0].end' }, typed), false);
	// the start is not what diverges
	strictEqual(claims('rest_param_type_end', { ...end, path: 'params[1].start' }, typed), false);
});

Deno.test('comment_dedup: the canonical comment array is the LONGER one', () => {
	const entry = (ours: number, canonical: number): Entry => ({
		kind: 'length_mismatch',
		path: 'body[0].leadingComments',
		ours,
		canonical
	});
	strictEqual(claims('comment_dedup', entry(1, 2), {}), true);
	// tsv emitting MORE comments is not a duplication on acorn's side
	strictEqual(claims('comment_dedup', entry(2, 1), {}), false);
	// a root `comments[i]` drift needs a duplicated span on the canonical side
	const drift: Entry = { kind: 'value_mismatch', path: 'comments[1].start', ours: 9, canonical: 4 };
	const duplicated = { comments: [{ start: 4 }, { start: 4 }, { start: 9 }] };
	strictEqual(claims('comment_dedup', drift, duplicated), true);
	strictEqual(claims('comment_dedup', drift, { comments: [{ start: 4 }, { start: 9 }] }), false);
});

Deno.test('svelte_template_paren_comment: a template-only attachment Svelte lacks', () => {
	const entry = (kind: Entry['kind'], path: string): Entry => ({
		kind,
		path,
		ours: [{}],
		canonical: undefined
	});
	const path = 'fragment.nodes[0].expression.left.leadingComments';
	strictEqual(
		claims('svelte_template_paren_comment', entry('missing_canonical', path), {}, 'svelte'),
		true
	);
	// a `<script>` parse sets no preserveParens, so the same attachment there is a bug
	const script = 'instance.content.body[0].expression.left.leadingComments';
	strictEqual(
		claims('svelte_template_paren_comment', entry('missing_canonical', script), {}, 'svelte'),
		false
	);
	// the direction: a comment tsv LACKS is a drop
	strictEqual(
		claims('svelte_template_paren_comment', entry('missing_ours', path), {}, 'svelte'),
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

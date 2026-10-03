/**
 * The documented parse divergences: the matchers that classify a raw AST difference
 * (`parse_diff.ts`'s `diff_asts`) as one of the AST-content divergences cataloged in
 * docs/conformance_svelte.md, each naming its section.
 *
 * Classification happens at the REPORTING layer, after the raw diff — never as pre-diff
 * normalization — so a bug in this reasoning surfaces as an undocumented group instead of
 * being silently absorbed. The matchers cover only the divergences that parse successfully
 * on both sides (the parser-FEATURE corrections in that doc — `using`, v-flag regex, CSS
 * namespaces — make the canonical parser throw, so they land in the error buckets instead).
 * When triage confirms a new group is a documented divergence, add a matcher here AND
 * ensure the divergence is cataloged in conformance_svelte.md.
 *
 * Shared by `corpus_compare_parse.ts` and the parse-conformance fixtures gates
 * (`fixtures_gate.ts`), so a divergence cataloged here shrinks every count at once.
 * Node-modules-free, so `parse_divergences_test.ts` gates it in `deno task test:deno`.
 *
 * @module
 */

import { get_at_path } from './diff_path.ts';
import type { DiffEntry, MatchContext } from './parse_diff.ts';

/** Svelte's `Nth.value` tell for an `of S` argument — see `subtree_has_nth_of`. */
const NTH_OF_TELL = /\sof\s*$/;

/**
 * True if `node`'s subtree contains an `Nth` whose value ends in Svelte's `of` word
 * (a normal Nth value never does) — the tell of a `:nth-child(An+B of S)` argument.
 * The word is followed by a space when the source spaces it (`2n of `) and by nothing
 * when the selector is glued to it (`2n of.a` → `2n of`).
 */
function subtree_has_nth_of(node: unknown): boolean {
	if (node == null || typeof node !== 'object') return false;
	const n = node as Record<string, unknown>;
	if (n.type === 'Nth' && typeof n.value === 'string' && NTH_OF_TELL.test(n.value)) return true;
	for (const v of Object.values(n)) {
		if (Array.isArray(v)) {
			if (v.some(subtree_has_nth_of)) return true;
		} else if (v && typeof v === 'object' && subtree_has_nth_of(v)) {
			return true;
		}
	}
	return false;
}

/** True if `property` carries a tell of Svelte's `read_declaration` tokenization garbage. */
const is_garbage_property = (property: unknown): boolean =>
	typeof property === 'string' && (property.startsWith(';') || property.includes('/*'));

/**
 * True if `node`'s subtree contains a canonical `Declaration` whose `property`
 * shows that garbage — the document-level precondition for the comment-array
 * consequence below.
 */
function subtree_has_garbage_declaration(node: unknown): boolean {
	if (node == null || typeof node !== 'object') return false;
	const n = node as Record<string, unknown>;
	if (n.type === 'Declaration' && is_garbage_property(n.property)) return true;
	for (const v of Object.values(n)) {
		if (Array.isArray(v)) {
			if (v.some(subtree_has_garbage_declaration)) return true;
		} else if (v && typeof v === 'object' && subtree_has_garbage_declaration(v)) {
			return true;
		}
	}
	return false;
}

/** A `<script>` open tag — `read_script` hands acorn everything before its body blanked. */
const SCRIPT_OPEN_TAG = /<script\b[^>]*>/gi;

/**
 * A block or tag head whose binding `read_pattern` / `read_type_annotation` read, or a
 * `{#snippet}` head (whose prelude blanks the non-whitespace) — the template constructs whose
 * acorn input Svelte manufactures.
 */
const MANUFACTURED_HEAD = /\{\s*(@const|#each|#await|:then|:catch|#snippet)\b/g;

/**
 * A markup-level HTML comment (blanked by `blank_html_comments`), or a `<script>` / `<style>`
 * element whose raw text is matched only so a `<!--` inside it is not read as one. Either may
 * run to the end of the text, where the comment being graded sits inside it.
 */
const HTML_COMMENT_OR_RAW_TEXT =
	/<!--[\s\S]*?(?:-->|$)|<(script|style)\b[^>]*>[\s\S]*?(?:<\/\1\s*>|$)/gi;

/**
 * `text` with every markup-level HTML comment blanked to spaces, so a `<script>` or a block head
 * spelled inside one — text Svelte never parses — is not read as one. The length is kept, so
 * every offset into the result is an offset into `text`.
 */
function blank_html_comments(text: string): string {
	return text.replace(HTML_COMMENT_OR_RAW_TEXT, (m, raw_text) =>
		raw_text === undefined ? ' '.repeat(m.length) : m
	);
}

/**
 * A head's binding keyword — `#each`'s `as`, `#await`'s `then` / `catch` — as a token of its own:
 * not part of a longer identifier, and not a member name (`p.then`, `a?.as`, `a. as`).
 */
const keyword_token = (alternatives: string): RegExp =>
	new RegExp(`(?<![\\w$])(?<!\\.\\s*)(?:${alternatives})(?![\\w$])`, 'g');
const EACH_BINDING_KEYWORD = keyword_token('as');
const AWAIT_BINDING_KEYWORD = keyword_token('then|catch');

/** The last match of `re` (a `g` regex) in `text`, or `null`. */
function last_match(re: RegExp, text: string): RegExpMatchArray | null {
	let last: RegExpMatchArray | null = null;
	for (const m of text.matchAll(re)) last = m;
	return last;
}

/**
 * Where each depth-0 character of `text` sits — the offsets (into `text`) at which the bracket
 * depth, counted from 0 at its start, is 0 — or `null` once a closer takes it below 0 (the
 * construct `text` sits in has closed).
 */
function depth_zero_offsets(text: string): number[] | null {
	const offsets: number[] = [];
	let depth = 0;
	for (let i = 0; i < text.length; i++) {
		const ch = text[i];
		if (ch === '(' || ch === '[' || ch === '{') {
			if (depth === 0) offsets.push(i);
			depth++;
		} else if (ch === ')' || ch === ']' || ch === '}') {
			depth--;
			if (depth < 0) return null;
		} else if (depth === 0) {
			offsets.push(i);
		}
	}
	return offsets;
}

/**
 * Where the text Svelte manufactured for the acorn parse holding the comment at `start` ends —
 * one past the last byte acorn did not read off the document — or `null` when that parse reads
 * the raw template (or no parse Svelte manufactures for holds the comment).
 *
 * - inside a `<script>`: the end of its open tag;
 * - in a `{#snippet}` head's parameters: the `(` / `<` they open with;
 * - in a block binding (`{@const}`'s id, past `{#each}`'s `as`, past `{#await}`'s `then` /
 *   `catch`, `{:then}` / `{:catch}`): the binding's first byte — or, past a depth-0 `:` (the
 *   binding's annotation, a second parse), one past that colon. `{@const}`'s initializer and
 *   `{#each}`'s `, index` / `(key)` tail are read off the raw template, so a comment there is
 *   no manufactured parse's.
 */
function manufactured_end(source: string, start: number): number | null {
	const prefix = blank_html_comments(source.slice(0, start));
	const script = last_match(SCRIPT_OPEN_TAG, prefix);
	if (script?.index !== undefined) {
		const end = script.index + script[0].length;
		if (!/<\/script/i.test(prefix.slice(end))) return end;
	}
	const head = last_match(MANUFACTURED_HEAD, prefix);
	if (head?.index === undefined) return null;
	const head_end = head.index + head[0].length;
	const rest = prefix.slice(head_end);
	// The head must still be open at the comment: no closer takes it below its own `{`.
	const top = depth_zero_offsets(rest);
	if (top === null) return null;
	const keyword = head[1];
	if (keyword === '#snippet') {
		const params = rest.search(/[(<]/);
		return params < 0 ? null : head_end + params;
	}
	let binding = 0;
	if (keyword === '#each' || keyword === '#await') {
		const separator = keyword === '#each' ? EACH_BINDING_KEYWORD : AWAIT_BINDING_KEYWORD;
		const m = last_match(separator, rest);
		if (m?.index === undefined || !top.includes(m.index)) return null;
		binding = m.index + m[0].length;
	}
	binding += /^\s*/.exec(rest.slice(binding))![0].length;
	let end = head_end + binding;
	for (const at of top) {
		if (at < binding) continue;
		const ch = rest[at];
		const next = rest[at + 1] ?? '';
		if (ch === '=' && keyword === '@const' && !'=>'.includes(next)) return null;
		if (ch === ',' || (ch === '(' && keyword === '#each')) return null;
		if (ch === ':') end = head_end + at + 1;
	}
	return end;
}

/**
 * Whether the comment at `start` opens on the line where the text Svelte manufactured for its
 * parse ends (`manufactured_end`) — the only line its `onComment` dedent can measure
 * differently from the document's. A `\n` between the two puts the comment on a line the
 * document's own bytes open, where both parsers measure the same run.
 */
function comment_opens_on_manufactured_line(source: string, start: number): boolean {
	const end = manufactured_end(source, start);
	return end !== null && end <= start && !source.slice(end, start).includes('\n');
}

/**
 * `onComment`'s strip (`svelte/src/compiler/phases/1-parse/acorn.js`): one copy of
 * `indentation` off the start of the value and after every ECMAScript line terminator — an
 * `m`-mode `^`, which is the class JS itself spells.
 */
function strip_comment_indentation(value: string, indentation: string): string {
	return indentation === '' ? value : value.replace(new RegExp(`^${indentation}`, 'gm'), '');
}

/**
 * Whether a comment-`value` difference is exactly the manufactured-line dedent divergence:
 * a multi-line Block comment opening on a manufactured line, where OUR value is its own
 * source text dedented by the `[ \t]` run its DOCUMENT line opens with (tsv's rule), and
 * canonical's is the same text dedented by some other `[ \t]` run (the one Svelte measured
 * in the string it handed acorn) — or not at all, where that run matched no line. A value
 * that is not a uniform dedent of the comment's own text, on either side, stays undocumented.
 */
function is_manufactured_line_dedent(
	entry: Omit<DiffEntry, 'documented' | 'signature'>,
	canonical_parent: unknown,
	source: string
): boolean {
	if (entry.kind !== 'value_mismatch') return false;
	if (!/(^|\.)(comments|leadingComments|trailingComments)\[\d+\]\.value$/.test(entry.path)) {
		return false;
	}
	if (typeof entry.ours !== 'string' || typeof entry.canonical !== 'string') return false;
	const comment = canonical_parent as { type?: unknown; start?: unknown; end?: unknown } | null;
	if (comment?.type !== 'Block') return false;
	const { start, end } = comment;
	if (typeof start !== 'number' || typeof end !== 'number') return false;
	if (source.slice(start, start + 2) !== '/*' || source.slice(end - 2, end) !== '*/') return false;
	const raw = source.slice(start + 2, end - 2);
	if (!raw.includes('\n')) return false;
	// tsv's side: the document line's own run, found by the same `\n`-only walk-back.
	const line_start = source.lastIndexOf('\n', start - 1) + 1;
	const document_run = /^[ \t]*/.exec(source.slice(line_start, start))![0];
	if (entry.ours !== strip_comment_indentation(raw, document_run)) return false;
	if (!canonical_is_uniform_dedent(raw, entry.canonical, document_run)) return false;
	// Last, as the costliest: the line must be one a manufacture ends on.
	return comment_opens_on_manufactured_line(source, start);
}

/**
 * Whether `canonical` is `raw` under a uniform `[ \t]` dedent other than `document_run` — or
 * under none at all, where the run Svelte measured matched no line. Any other run is a prefix
 * of the run one of the text's own lines opens with, so those prefixes are every candidate.
 */
function canonical_is_uniform_dedent(
	raw: string,
	canonical: string,
	document_run: string
): boolean {
	if (canonical === raw) return true;
	for (const line of raw.split(/\r\n?|[\n\u2028\u2029]/)) {
		const run = /^[ \t]*/.exec(line)![0];
		for (let len = 1; len <= run.length; len++) {
			const candidate = run.slice(0, len);
			if (candidate !== document_run && strip_comment_indentation(raw, candidate) === canonical) {
				return true;
			}
		}
	}
	return false;
}

export interface DocumentedMatcher {
	name: string;
	/** docs/conformance_svelte.md section the divergence is cataloged under */
	conformance_section: string;
	matches: (
		entry: Omit<DiffEntry, 'documented' | 'signature'>,
		canonical_parent: unknown,
		ctx: MatchContext
	) => boolean;
}

export const DOCUMENTED_MATCHERS: DocumentedMatcher[] = [
	{
		// Acorn-typescript's backtrack-reparse duplicates a comment inside any
		// re-parsed construct, emitting it twice — into a node's
		// leading/trailingComments AND into the root `comments` array (which
		// shifts every later index). tsv emits each comment once, so the canonical
		// side always has MORE entries. Two precise signatures (NOT "any path with
		// a comment field" — that masked genuine attachment divergences):
		// (1) a comment array that is strictly longer on the canonical side, and
		// (2) a root `comments[i]` field drift, gated on canonical actually
		// carrying a duplicated comment span (so a real per-comment value/offset
		// bug, with no duplicate, still surfaces as undocumented).
		name: 'comment_dedup',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry, canonical_parent, ctx) => {
			if (
				entry.kind === 'length_mismatch' &&
				/(^|\.)(comments|leadingComments|trailingComments)$/.test(entry.path) &&
				Number(entry.canonical) > Number(entry.ours)
			) {
				return true;
			}
			if (entry.kind === 'value_mismatch' && /^comments\[\d+\]\./.test(entry.path)) {
				const root = ctx.canonical_root as { comments?: { start?: number }[] } | null;
				const starts = root?.comments?.map((c) => c?.start);
				return Array.isArray(starts) && new Set(starts).size !== starts.length;
			}
			// (3) An ATTACHED copy whose `value` disagrees with its own twin. The throwaway
			// parse is entered over a DIFFERENT synthetic source than the surviving one, and
			// the comment `value` acorn writes is dedented by the indentation of the comment's
			// line IN that source — so the two copies of one multi-line block comment can carry
			// two different values, and index [0] is the discarded parse's. Gated hard on the
			// twin actually existing and on OUR value being one of the twins', so a genuine
			// dedent bug — a value neither copy holds — stays undocumented.
			const attached = /^(.*(?:leadingComments|trailingComments))\[\d+\]\.value$/.exec(entry.path);
			if (entry.kind === 'value_mismatch' && attached) {
				const arr = get_at_path(ctx.canonical_root, attached[1]) as
					{ start?: number; end?: number; value?: unknown }[] | null;
				const copy = canonical_parent as { start?: number; end?: number } | null;
				if (!Array.isArray(arr) || copy == null || typeof copy.start !== 'number') {
					return false;
				}
				const twins = arr.filter((c) => c?.start === copy.start && c?.end === copy.end);
				return twins.length > 1 && twins.some((c) => c?.value === entry.ours);
			}
			return false;
		}
	},
	{
		// Svelte's `onComment` dedents a multi-line block comment by the `[ \t]` run opening
		// its line IN THE STRING ACORN WAS HANDED, and four readers manufacture that string
		// (`read_script`'s blanked prefix, `read_pattern`'s `(pattern = 1)`, the `_ as `
		// insert, the `{#snippet}` prelude) — so on the line a manufacture ends, Svelte
		// measures a run the document does not hold. tsv measures the document's line. Gated
		// on both values being a uniform dedent of the comment's own text (ours by exactly
		// the document run) and on the comment opening on such a line, so any other `value`
		// difference stays undocumented.
		name: 'manufactured_line_comment_dedent',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry, canonical_parent, ctx) =>
			is_manufactured_line_dedent(entry, canonical_parent, ctx.source)
	},
	{
		// Svelte parses `<script module>` and the instance `<script>` against one
		// shared root.comments queue (acorn.js get_comment_handlers/add_comments),
		// so a module-region comment — a module-script comment, or a leading
		// fragment HTML comment (`<!-- @component -->`) — is also attached to the
		// instance script: ahead of its first statement's own leading comments, or,
		// when the script holds no statement, ahead of the Program's own trailing
		// comments (`add_comments`'s root special case). tsv attaches each comment
		// once, in its source region. The comment is never lost (it stays on its
		// module/fragment home), so this is a pure cross-script duplication.
		// Two shapes: (1) tsv has no comment array at that slot, so the shifted
		// comment is a plain `missing_ours` on the array; (2) tsv DOES have its own
		// comments there, and the copies form a PREFIX of the canonical array — the
		// module-region comments in source order, then the instance's own — so
		// every index reads one whole prefix later: canonical `[i]` is tsv's
		// `[i − shift]`, a `value_mismatch` on each of the comment's fields at EVERY
		// index, not only `[0]`. The arm counts that prefix (the canonical comments
		// whose span lies BEFORE the instance script) and admits an entry only when
		// OUR value is the same field of canonical's comment at `[i + shift]` — so a
		// genuine instance-internal attachment difference (a comment tsv lists at
		// `[i]` that canonical never lists after the prefix) stays undocumented.
		name: 'svelte_instance_comment_duplication',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry, _canonical_parent, ctx) => {
			if (
				entry.kind === 'missing_ours' &&
				/^instance\.content(\.body\[0\])?\.(leadingComments|trailingComments)$/.test(entry.path)
			) {
				return true;
			}
			const shifted =
				/^(instance\.content(?:\.body\[0\])?\.(?:leadingComments|trailingComments))\[(\d+)\]\.(type|value|start|end)$/.exec(
					entry.path
				);
			if (entry.kind === 'value_mismatch' && shifted) {
				const [, array_path, index, field] = shifted;
				const arr = get_at_path(ctx.canonical_root, array_path) as Record<string, unknown>[] | null;
				const instance = get_at_path(ctx.canonical_root, 'instance.content') as {
					start?: number;
				} | null;
				if (!Array.isArray(arr) || instance == null || typeof instance.start !== 'number') {
					return false;
				}
				const instance_start = instance.start;
				const in_module_region = (comment: Record<string, unknown> | undefined): boolean =>
					typeof comment?.start === 'number' && comment.start < instance_start;
				let shift = 0;
				while (in_module_region(arr[shift])) shift++;
				if (shift === 0) return false;
				const twin = arr[Number(index) + shift];
				return twin != null && twin[field] === entry.ours;
			}
			return false;
		}
	},
	{
		// Svelte's parse_expression_at sets acorn `preserveParens: true`; a leading
		// comment before a parenthesized subexpression attaches to the synthetic
		// ParenthesizedExpression, which Svelte's remove_parens then strips —
		// dropping the attachment (the comment survives only in root `comments`).
		// tsv has no ParenthesizedExpression node, so it keeps the comment on the
		// inner expression — a template-expression attachment Svelte lacks
		// (`missing_canonical` under a `fragment.` path). Template-only; a plain
		// `<script>` parse does not set preserveParens.
		name: 'svelte_template_paren_comment',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry) =>
			entry.kind === 'missing_canonical' &&
			/^fragment\./.test(entry.path) &&
			/(^|\.)(leadingComments|trailingComments)$/.test(entry.path)
	},
	{
		// acorn-typescript ends a typed RestElement at the binding, excluding the
		// type annotation (`(...args: Array<any>)` → end after `args`) — inconsistent
		// with its own Identifier params, and with babel/TS-ESLint, which include
		// the annotation like tsv does.
		name: 'rest_param_type_end',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — Rest param type-annotation end',
		matches: (entry, _canonical_parent, ctx) => {
			const m = entry.path.match(/^(.*)\.end$/);
			if (!m) return false;
			const owner = get_at_path(ctx.canonical_root, m[1]) as {
				type?: unknown;
				typeAnnotation?: unknown;
			} | null;
			return owner?.type === 'RestElement' && owner.typeAnnotation != null;
		}
	},
	{
		// `static` newline `static` in a class body: tsc reads modifier + member (a
		// static field named `static`); acorn ASI-splits every bare `static` into its
		// own value-less field. tsv follows tsc. Scoped to class bodies whose
		// canonical AST contains a value-less, non-computed property literally named
		// `static` — the ladder pattern.
		name: 'static_member_ladder',
		conformance_section: 'TypeScript Parser Corrections (corpus-enforced) — static member ladder',
		matches: (entry, _canonical_parent, ctx) => {
			const segments = entry.path.split('.');
			for (let i = segments.length - 1; i > 0; i--) {
				const node = get_at_path(ctx.canonical_root, segments.slice(0, i).join('.')) as {
					type?: unknown;
					body?: unknown;
				} | null;
				if (node?.type !== 'ClassBody' || !Array.isArray(node.body)) continue;
				return node.body.some(
					(member: {
						type?: unknown;
						computed?: unknown;
						value?: unknown;
						key?: { name?: unknown };
					}) =>
						member?.type === 'PropertyDefinition' &&
						member.computed === false &&
						member.value === null &&
						member.key?.name === 'static'
				);
			}
			return false;
		}
	},
	{
		// acorn-typescript leaves a class heritage with type args as a
		// `TSInstantiationExpression` superClass when a line break precedes the next
		// clause (`extends Base<T>` newline `implements I` — its instantiation bail
		// checks hasPrecedingLineBreak); the same-line form yields
		// `superClass: Identifier` + `superTypeParameters`. tsv emits the same-line
		// shape uniformly.
		name: 'extends_instantiation_linebreak',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — extends instantiation line-break shape',
		matches: (entry, _canonical_parent, ctx) => {
			const m = entry.path.match(/^(.*)\.(?:superClass(?:\..*)?|superTypeParameters)$/);
			if (!m) return false;
			const cls = get_at_path(ctx.canonical_root, m[1]) as {
				superClass?: { type?: unknown } | null;
			} | null;
			return cls?.superClass?.type === 'TSInstantiationExpression';
		}
	},
	{
		// A lone UTF-16 surrogate in a string value (`"\ud800"`) is unrepresentable
		// in tsv's Rust strings — the decoded value carries U+FFFD where acorn keeps
		// the WTF-16 lone surrogate. Matches when replacing canonical's lone
		// surrogates with U+FFFD yields ours.
		name: 'lone_surrogate_value',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — Lone surrogates in string values',
		matches: (entry) => {
			if (entry.kind !== 'value_mismatch') return false;
			if (typeof entry.ours !== 'string' || typeof entry.canonical !== 'string') return false;
			const replaced = entry.canonical.replace(
				/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/gu,
				'\u{FFFD}'
			);
			return replaced !== entry.canonical && replaced === entry.ours;
		}
	},
	{
		// acorn-typescript starts the call/member nodes built on a parenthesized
		// decorator expression after the opening paren (`@(a)() p` → start at the
		// inner `a`) — inconsistent with its own non-decorator parse of `(a)()`
		// and with babel, which start at the `(` like tsv does.
		name: 'decorator_paren_subscript_start',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — Parenthesized decorator subscript start',
		matches: (entry, _canonical_parent, ctx) => {
			const m = entry.path.match(
				/^(.*\.decorators\[\d+\]\.expression)(?:\.(?:callee|object|expression))*\.start$/
			);
			if (!m) return false;
			const expr = get_at_path(ctx.canonical_root, m[1]) as { type?: unknown } | null;
			return expr?.type === 'CallExpression' || expr?.type === 'MemberExpression';
		}
	},
	{
		// Svelte's read_declaration tokenizes garbage when a stray `;` or an adjacent
		// comment touches the property name: `border-box;;` yields a declaration with
		// property ";" that swallows the next declaration into its value, and
		// `color/* c */:` yields property "color/*" with the comment tail leaking into
		// the value (read_until stops at the first whitespace, which sits INSIDE the
		// comment). tsv parses per spec (skips empty declarations, tokenizes comments).
		//
		// Two shapes, because the same garbage lands in two places. (1) The
		// declaration's own `property`/`value`, gated on the canonical PARENT being
		// the garbage declaration. (2) The stylesheet's flat `comments` array: a
		// comment Svelte swallowed into a property token is never captured, so tsv
		// emits it plus every later comment at a shifted index, with `value` and
		// `position` following the shift. One insertion renumbers the whole tail, so
		// that arm cannot be scoped per index — it is scoped to the root `comments`
		// array of a document that actually contains the garbage. It is scoped by
		// DIRECTION too (the `comment_dedup` discipline, mirrored): the divergence
		// is an INSERTION, so the array must be longer on OUR side. A comment tsv
		// *loses* — the failure that matters — is the opposite direction, and its
		// `length_mismatch` still surfaces undocumented, as does a `missing_ours`
		// on the array ITSELF (tsv emitting no comments array at all) and any
		// comment-array divergence in a file with no garbage declaration.
		//
		// ⚠️ The direction lives on the ARRAY, never on the entry kind. Inside an
		// element the shift can only reach the FIELDS — an element always pairs
		// with an element — and `position` is the one OPTIONAL field (Svelte sets
		// it on a `read_value` comment only), so a value comment paired with a
		// non-value one reports it missing on whichever side lacks it. That is why
		// all three of `value_mismatch` / `missing_canonical` / `missing_ours` land
		// on `comments[i].position`, and why the refusal below is keyed on the PATH
		// rather than the kind — keyed on the kind it claims one renumbering and
		// refuses another that is the same fact told from the other side.
		name: 'css_declaration_tokenization',
		conformance_section:
			'CSS Parser Corrections (corpus-enforced) — Declaration tokenization garbage',
		matches: (entry, canonical_parent, ctx) => {
			const parent = canonical_parent as { type?: unknown; property?: unknown } | null | undefined;
			if (parent?.type === 'Declaration' && is_garbage_property(parent.property)) return true;
			if (!/^(css\.)?comments(\[\d+\])?(\.|$)/.test(entry.path)) return false;
			if (entry.kind === 'length_mismatch' && Number(entry.ours) <= Number(entry.canonical)) {
				return false;
			}
			if (entry.kind === 'missing_ours' && !/\[\d+\]\.position$/.test(entry.path)) return false;
			return subtree_has_garbage_declaration(ctx.canonical_root);
		}
	},
	{
		// acorn-typescript reads `<T>(<parenthesized arrow>)` as a generic arrow —
		// its Babel-ported abort on parenthesized arrows never fires (acorn sets no
		// `extra.parenthesized`) — where TypeScript reads a type assertion over the
		// arrow. tsv follows TypeScript (`TSTypeAssertion` wrapping the arrow), so
		// every field of that expression diffs. Gate: some ancestor canonical node
		// is an arrow with `typeParameters` whose source continues `( (` after the
		// type-params `>` — a doubled paren cannot open a real generic arrow's
		// param list, so genuine generic-arrow divergences stay unmasked.
		name: 'type_assertion_paren_arrow',
		conformance_section: 'TypeScript Corrections — Type assertion vs. generic arrow',
		matches: (entry, _canonical_parent, ctx) => {
			const parts = entry.path.split('.');
			for (let i = parts.length - 1; i >= 1; i--) {
				const node = get_at_path(ctx.canonical_root, parts.slice(0, i).join('.')) as {
					type?: unknown;
					typeParameters?: { end?: unknown } | null;
				} | null;
				if (
					node?.type === 'ArrowFunctionExpression' &&
					typeof node.typeParameters?.end === 'number' &&
					/^\s*\(\s*\(/.test(ctx.source.slice(node.typeParameters.end))
				) {
					return true;
				}
			}
			return false;
		}
	},
	{
		// Svelte's parseCss reads `:nth-child(An+B of S)` as `Nth.value = "2n of "`
		// (the ` of ` leaks in from its REGEX_NTH_OF terminator) and flattens `S` as
		// sibling simple selectors of the Nth. Per Selectors 4 the `S` is a nested
		// <complex-selector-list> scoped to the nth term, so tsv keeps `Nth.value =
		// "2n"` with `S` nested under a tsv-only `Nth.selector` field. The whole args
		// subtree reshapes: the `Nth.value`/`.selector`, the sibling-count lengths, and
		// the container span ends all differ. Anchor on Svelte's unambiguous tell — a
		// canonical `Nth` whose value ends in the `of` word — and absorb only the
		// reshape-shaped fields, so a genuine content bug inside `S` (a wrong
		// `.name`/`.type`) still surfaces as undocumented.
		name: 'nth_of_structure',
		conformance_section: 'CSS Corrections — :nth-child(An+B of S)',
		matches: (entry, _canonical_parent, ctx) => {
			const args_match = entry.path.match(/^(.*\.args)\b/);
			if (!args_match) return false;
			if (!subtree_has_nth_of(get_at_path(ctx.canonical_root, args_match[1]))) return false;
			return (
				(entry.kind === 'value_mismatch' && /\.(value|start|end)$/.test(entry.path)) ||
				(entry.kind === 'missing_canonical' && /\.selector$/.test(entry.path)) ||
				(entry.kind === 'length_mismatch' && /\.(children|selectors)$/.test(entry.path))
			);
		}
	}
];

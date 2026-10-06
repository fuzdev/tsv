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
import { wire_text } from './loc_tolerance.ts';
import type { DiffEntry, MatchContext } from './parse_diff.ts';

type Entry = Omit<DiffEntry, 'documented' | 'signature'>;

/**
 * The text the document's wire offsets index: for TypeScript the file itself (acorn counts a
 * leading BOM), for Svelte and CSS the string with a leading BOM removed (`parse` and
 * `parseCss` strip it before parsing, the acorn islands included) — `wire_text`'s reading,
 * which the `loc` tolerance rows share. A matcher that reads source bytes at a wire offset
 * reads them here, never in `ctx.source`, or a BOM-led Svelte file reads every byte one
 * unit late.
 */
function offset_text(ctx: MatchContext): string {
	return ctx.language === 'typescript' ? ctx.source : wire_text(ctx.source);
}

/** Whether two JSON values are deep-equal, object key order ignored. */
function json_equal(a: unknown, b: unknown): boolean {
	if (a === b) return true;
	if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
	if (Array.isArray(a) !== Array.isArray(b)) return false;
	const ao = a as Record<string, unknown>;
	const bo = b as Record<string, unknown>;
	const keys = Object.keys(ao);
	if (keys.length !== Object.keys(bo).length) return false;
	return keys.every((key) => key in bo && json_equal(ao[key], bo[key]));
}

/** A comment as the matchers read it — every field optional, since the tree is untyped. */
interface CommentLike {
	start?: unknown;
	end?: unknown;
	value?: unknown;
	[field: string]: unknown;
}

/**
 * `comments`' distinct spans, each with every copy of it the array holds, in source order —
 * the list a parser that emits each comment once writes. A duplicate is a later entry with
 * an earlier entry's `start` and `end`; its `value` may differ (the copies' dedents can). An
 * entry without a numeric span (Svelte's copy of an HTML comment) is never a duplicate.
 */
function distinct_comment_spans(comments: readonly unknown[]): CommentLike[][] {
	const by_span = new Map<unknown, CommentLike[]>();
	for (const c of comments) {
		const comment = (c ?? {}) as CommentLike;
		const spanned = typeof comment.start === 'number' && typeof comment.end === 'number';
		const key = spanned ? `${comment.start}:${comment.end}` : comment;
		const copies = by_span.get(key);
		if (copies) copies.push(comment);
		else by_span.set(key, [comment]);
	}
	return [...by_span.values()].sort((a, b) => Number(a[0]!.start) - Number(b[0]!.start) || 0);
}

/** Whether the gap from `at` to the next token — whitespace and comments — holds a line break. */
function line_break_follows(text: string, at: number): boolean {
	const gap = /(?:\s|\/\/[^\n\r\u2028\u2029]*|\/\*[\s\S]*?\*\/)*/y;
	gap.lastIndex = at;
	return /[\n\r\u2028\u2029]/.test(gap.exec(text)![0]);
}

/** A run of whitespace and comments, and nothing else. */
// a block comment's body never crosses a `*/`, so the anchored run cannot stretch one
// comment over the code between two
const COMMENT_RUN = /^(?:\s|\/\/[^\n\r\u2028\u2029]*|\/\*(?:[^*]|\*(?!\/))*\*\/)*$/;

const attached_spans_by_root = new WeakMap<object, Set<string>>();

/**
 * The `start:end` span of every comment the tree attaches — each entry of a
 * `leadingComments` or `trailingComments` array anywhere in it — memoized per tree.
 */
function attached_comment_spans(root: unknown): Set<string> {
	if (root == null || typeof root !== 'object') return new Set();
	const cached = attached_spans_by_root.get(root);
	if (cached) return cached;
	const spans = new Set<string>();
	const walk = (node: unknown): void => {
		if (node == null || typeof node !== 'object') return;
		if (Array.isArray(node)) {
			for (const child of node) walk(child);
			return;
		}
		for (const [key, value] of Object.entries(node)) {
			if ((key === 'leadingComments' || key === 'trailingComments') && Array.isArray(value)) {
				for (const c of value as (CommentLike | null)[]) {
					if (typeof c?.start === 'number') spans.add(`${c.start}:${c.end}`);
				}
			}
			walk(value);
		}
	};
	walk(root);
	attached_spans_by_root.set(root, spans);
	return spans;
}

/** The offset of the last non-whitespace unit before `at`, or `-1`. */
function preceding_token_end(text: string, at: number): number {
	let i = at - 1;
	while (i >= 0 && /\s/.test(text[i]!)) i--;
	return i;
}

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
 * The earliest `start` of a canonical `Declaration` in `node`'s subtree whose `property`
 * shows that garbage, or `null` when it holds none — where the comment-array consequence
 * below can begin.
 */
function first_garbage_declaration_start(node: unknown): number | null {
	if (node == null || typeof node !== 'object') return null;
	const n = node as Record<string, unknown>;
	let first: number | null =
		n.type === 'Declaration' && is_garbage_property(n.property) && typeof n.start === 'number'
			? n.start
			: null;
	for (const v of Object.values(n)) {
		for (const child of Array.isArray(v) ? v : [v]) {
			const found = first_garbage_declaration_start(child);
			if (found !== null && (first === null || found < first)) first = found;
		}
	}
	return first;
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
/** The offset of the `)` closing the `(` at `open`, by paren depth alone; -1 if unclosed. */
function matching_close_paren(text: string, open: number): number {
	let depth = 0;
	for (let i = open; i < text.length; i++) {
		if (text[i] === '(') depth++;
		else if (text[i] === ')' && --depth === 0) return i;
	}
	return -1;
}

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
	entry: Entry,
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

/** A comment-array path's three spellings: the root list and the two attachment lists. */
const COMMENT_ARRAY = /(^|\.)(comments|leadingComments|trailingComments)$/;

/**
 * Where a class body's static member ladder starts in the canonical member list — the
 * first field acorn ASI-split off a bare `static`: a non-static, non-computed, value-less
 * field keyed `static` whose span ends at its key (no `;` — a written `static;` ends past
 * it) with a line break before the next member — or `-1` when the body holds none. The
 * split leaves the keyword no modifier to be, so a written `static static⏎`, a static
 * field of the same shape, is no ladder.
 */
function ladder_start(members: readonly unknown[], text: string): number {
	for (let i = 0; i + 1 < members.length; i++) {
		const member = members[i] as {
			type?: unknown;
			static?: unknown;
			computed?: unknown;
			value?: unknown;
			end?: unknown;
			key?: { type?: unknown; name?: unknown; end?: unknown } | null;
		} | null;
		const next = (members[i + 1] as { start?: unknown } | null)?.start;
		if (
			member?.type === 'PropertyDefinition' &&
			member.static === false &&
			member.computed === false &&
			member.value === null &&
			member.key?.type === 'Identifier' &&
			member.key.name === 'static' &&
			typeof member.end === 'number' &&
			member.end === member.key.end &&
			typeof next === 'number' &&
			/[\n\r\u2028\u2029]/.test(text.slice(member.end, next))
		) {
			return i;
		}
	}
	return -1;
}

/** The roots Svelte lifts out of the fragment: `<script module>`, `<script>`, `<style>`. */
type LiftedRoot = 'module' | 'instance' | 'css';
const LIFTED_ROOTS: readonly LiftedRoot[] = ['module', 'instance', 'css'];

interface FragmentNodeLike {
	type?: unknown;
	start?: unknown;
	end?: unknown;
	data?: unknown;
}

/**
 * The fragment `Comment` Svelte copies onto lifted root `key` past another lifted root, or
 * `null` — its own walk (`1-parse/state/element.js`) from the root's tag back through the
 * fragment: the last node before the tag must end AT it, whitespace-only `Text` is stepped
 * over, and the first `Comment` reached is the copy's source. The walk never sees a lifted
 * root (none is appended to the fragment), so where the nodes it steps between leave a gap,
 * the gap must be tiled exactly by the other lifted roots' spans, and at least one must be
 * — with none stepped over, `key` is the comment's nearest root, which tsv attaches too.
 */
function comment_copied_past_lifted_root(
	canonical_root: unknown,
	key: LiftedRoot
): FragmentNodeLike | null {
	const root = canonical_root as Record<string, unknown> | null;
	const target = root?.[key] as { start?: unknown } | null | undefined;
	const nodes = (root?.fragment as { nodes?: unknown } | null | undefined)?.nodes;
	if (typeof target?.start !== 'number' || !Array.isArray(nodes)) return null;
	const tag = target.start;
	const holes: { start: number; end: number }[] = [];
	for (const other of LIFTED_ROOTS) {
		const hole = other === key ? null : (root![other] as { start?: unknown; end?: unknown } | null);
		if (typeof hole?.start === 'number' && typeof hole.end === 'number') {
			holes.push({ start: hole.start, end: hole.end });
		}
	}
	const before = (nodes as (FragmentNodeLike | null)[]).filter(
		(n): n is FragmentNodeLike => typeof n?.end === 'number' && n.end <= tag
	);
	let at = tag;
	let stepped = 0;
	for (let i = before.length - 1; i >= 0; i--) {
		const node = before[i]!;
		if (typeof node.start !== 'number') return null;
		if (i < before.length - 1) {
			while (node.end !== at) {
				const hole = holes.find((h) => h.end === at);
				if (hole === undefined) return null;
				at = hole.start;
				stepped++;
			}
		} else if (node.end !== at) {
			return null;
		}
		if (node.type === 'Comment') return stepped > 0 && typeof node.data === 'string' ? node : null;
		if (node.type !== 'Text' || typeof node.data !== 'string' || node.data.trim() !== '') {
			return null;
		}
		at = node.start;
	}
	return null;
}

export interface DocumentedMatcher {
	name: string;
	/** docs/conformance_svelte.md section the divergence is cataloged under */
	conformance_section: string;
	matches: (entry: Entry, canonical_parent: unknown, ctx: MatchContext) => boolean;
}

export const DOCUMENTED_MATCHERS: DocumentedMatcher[] = [
	{
		// A parse Svelte discards after its `onComment` already ran (the typed `{@const}`
		// head's `_ as` parse, the `{#each}` head's speculative reads) pushes every comment
		// it scanned into the shared `root.comments`, and the surviving parse pushes them
		// again — so a comment is listed twice, in the root array (shifting every later
		// index) and in a node's leading/trailingComments. tsv emits each comment once.
		// Three precise signatures, each requiring the duplicate to EXIST (NOT "any path
		// with a comment field" — that masked genuine attachment divergences, and a
		// comment tsv drops is the failure this must never excuse):
		// (1) a comment array longer on the canonical side by exactly its duplicates —
		//     its distinct spans number what tsv lists;
		// (2) a root `comments[i]` field drift where OUR value is that field of the
		//     canonical root's i-th distinct comment (one of its copies) — the shift its
		//     duplicates put on index `i`, and nothing else;
		// (3) an attached copy's `value` that disagrees with its twin (below).
		name: 'comment_dedup',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry, canonical_parent, ctx) => {
			if (entry.kind === 'length_mismatch' && COMMENT_ARRAY.test(entry.path)) {
				const arr = get_at_path(ctx.canonical_root, entry.path);
				return (
					Array.isArray(arr) &&
					arr.length === entry.canonical &&
					Number(entry.canonical) > Number(entry.ours) &&
					distinct_comment_spans(arr).length === entry.ours
				);
			}
			const root_field = /^comments\[(\d+)\]\.([^.[]+)$/.exec(entry.path);
			if (entry.kind === 'value_mismatch' && root_field) {
				const comments = (ctx.canonical_root as { comments?: unknown } | null)?.comments;
				if (!Array.isArray(comments)) return false;
				const distinct = distinct_comment_spans(comments);
				if (distinct.length === comments.length) return false;
				const [, index, field] = root_field;
				return distinct[Number(index)]?.some((copy) => copy[field!] === entry.ours) ?? false;
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
		// its line IN THE STRING ACORN WAS HANDED, and two readers manufacture that string
		// (`read_script`'s blanked prefix, and the `_ as ` insert where it swallows the
		// newline before a binding's colon) — so on the line a manufacture ends, Svelte
		// measures a run the document does not hold. tsv measures the document's line. Gated
		// on both values being a uniform dedent of the comment's own text (ours by exactly
		// the document run) and on the comment opening on such a line, so any other `value`
		// difference stays undocumented.
		name: 'manufactured_line_comment_dedent',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry, canonical_parent, ctx) =>
			is_manufactured_line_dedent(entry, canonical_parent, offset_text(ctx))
	},
	{
		// Svelte parses `<script module>` and the instance `<script>` against one
		// shared root.comments queue (acorn.js get_comment_handlers/add_comments),
		// so a module-script comment is also attached to the instance script: ahead
		// of its first statement's own leading comments, or, when the script holds no
		// statement, ahead of the Program's own trailing comments (`add_comments`'s root
		// special case). tsv attaches each comment once, in its source region. The
		// comment is never lost (it stays on its module home), so this is a pure
		// cross-script duplication. (A leading HTML comment's span-less copy is the next
		// matcher's: it reaches the instance by another mechanism, and not only it.)
		// The copies form a PREFIX of the canonical array — the module-region comments
		// in source order, then the instance's own — and every arm is gated on that
		// prefix. A module-region comment is one whose span lies BEFORE the instance
		// script.
		// (1) tsv has no comment array at that slot (the instance has no comments of its
		//     own there), so the canonical array must be ALL prefix — a `missing_ours`
		//     on the array;
		// (2) tsv DOES have its own comments there, so the array is longer on the
		//     canonical side by exactly the prefix — a `length_mismatch`;
		// (3) and then every index reads one whole prefix later: canonical `[i]` is
		//     tsv's `[i − shift]`, a `value_mismatch` on each of the comment's fields at
		//     EVERY index, not only `[0]`. The arm admits an entry only when OUR value is
		//     the same field of canonical's comment at `[i + shift]` — so a genuine
		//     instance-internal attachment difference (a comment tsv lists at `[i]` that
		//     canonical never lists after the prefix) stays undocumented.
		name: 'svelte_instance_comment_duplication',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry, _canonical_parent, ctx) => {
			const at =
				/^(instance\.content(?:\.body\[0\])?\.(?:leadingComments|trailingComments))(?:\[(\d+)\]\.(type|value|start|end))?$/.exec(
					entry.path
				);
			if (!at) return false;
			const [, array_path, index, field] = at;
			const arr = get_at_path(ctx.canonical_root, array_path!);
			const instance = get_at_path(ctx.canonical_root, 'instance.content') as {
				start?: unknown;
			} | null;
			if (!Array.isArray(arr) || typeof instance?.start !== 'number') return false;
			const instance_start = instance.start;
			const in_module_region = (comment: unknown): boolean => {
				const start = (comment as CommentLike | undefined)?.start;
				return typeof start === 'number' && start < instance_start;
			};
			let shift = 0;
			while (in_module_region(arr[shift])) shift++;
			if (shift === 0) return false;
			if (index === undefined) {
				if (entry.kind === 'missing_ours') return shift === arr.length;
				return (
					entry.kind === 'length_mismatch' &&
					entry.canonical === arr.length &&
					arr.length - Number(entry.ours) === shift
				);
			}
			if (entry.kind !== 'value_mismatch') return false;
			const twin = arr[Number(index) + shift] as CommentLike | undefined;
			return twin != null && twin[field!] === entry.ours;
		}
	},
	{
		// A lifted `<script>` / `<style>` is never appended to the fragment, so the walk
		// back from each one for a preceding HTML comment (`1-parse/state/element.js`)
		// steps over every EARLIER lifted root as if it were absent, and a leading
		// `<!-- c -->` is copied onto every later root — a `<script>`'s Program as the
		// span-less `{type: 'Line', value}`, a `<style>`'s content as the `Comment` node
		// itself. tsv stops at the hole a lifted tag leaves, so only the nearest root
		// holds it; the comment stays a fragment node in both. Gated on Svelte's own walk
		// (`comment_copied_past_lifted_root`): from the root the copy sits on, back over
		// whitespace-only `Text` and the other lifted roots' spans to a `Comment` whose
		// copy it is, at least one root stepped over — so the nearest root's copy, the
		// one tsv owes, is never excused, even when an earlier comment has the same text.
		name: 'svelte_lifted_root_comment_duplication',
		conformance_section:
			'Comment Attachment Differences — Leading HTML comment duplicated onto every later lifted root',
		matches: (entry, _canonical_parent, ctx) => {
			if (ctx.language !== 'svelte') return false;
			if (entry.path === 'css.content.comment') {
				if (entry.kind !== 'type_mismatch' || entry.ours !== null) return false;
				const comment = comment_copied_past_lifted_root(ctx.canonical_root, 'css');
				return comment !== null && json_equal(entry.canonical, comment);
			}
			const script = /^(instance|module)\.content\.leadingComments$/.exec(entry.path);
			if (!script || entry.kind !== 'missing_ours') return false;
			const copies = entry.canonical;
			if (!Array.isArray(copies) || copies.length !== 1) return false;
			const comment = comment_copied_past_lifted_root(
				ctx.canonical_root,
				script[1] as 'instance' | 'module'
			);
			return comment !== null && json_equal(copies[0], { type: 'Line', value: comment.data });
		}
	},
	{
		// Svelte's parse_expression_at sets acorn `preserveParens: true`; a leading
		// comment before a parenthesized subexpression attaches to the synthetic
		// ParenthesizedExpression, which Svelte's remove_parens then strips —
		// dropping the attachment (the comment survives only in root `comments`).
		// tsv keeps no span for a bare pair inside an expression, so it keeps the comment
		// on the inner expression — a template-expression attachment Svelte lacks
		// (`missing_canonical` under a `fragment.` path). (A JSDoc cast's pair, and the
		// bare pairs around an island's root, run as discarded nodes of tsv's attach and
		// match.) Template-only; a plain `<script>` parse does not set preserveParens.
		// LEADING only — the trailing side of an inner bare pair (`{[(a) /* c */ /* d */]}`)
		// is the same divergence but unclaimed here, so a real case would gate. Gated on the shape itself: the node
		// tsv attached to is the inside of a paren — the first token before its start
		// is a `(` — tsv's comments are the run directly ahead of that `(` (nothing but
		// whitespace and comments from the first of them to it, so a call's `(` after a
		// callee is refused — and exactly that run, in order, each once), and canonical
		// attached none of them anywhere — the wrapper's
		// comments survive only in the root `comments` array, which is what removing it
		// leaves.
		name: 'svelte_template_paren_comment',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry, canonical_parent, ctx) => {
			if (entry.kind !== 'missing_canonical') return false;
			if (!/^fragment\..*\.leadingComments$/.test(entry.path)) return false;
			const start = (canonical_parent as { start?: unknown } | null)?.start;
			if (typeof start !== 'number' || !Array.isArray(entry.ours) || entry.ours.length === 0) {
				return false;
			}
			const comments = entry.ours as (CommentLike | null)[];
			if (!comments.every((c) => typeof c?.start === 'number' && typeof c.end === 'number')) {
				return false;
			}
			const text = offset_text(ctx);
			const paren = preceding_token_end(text, start);
			if (text[paren] !== '(') return false;
			// in source order, each once, ending before the `(`
			for (let i = 1; i < comments.length; i++) {
				if ((comments[i]!.start as number) < (comments[i - 1]!.end as number)) return false;
			}
			const first = comments[0]!.start as number;
			if ((comments.at(-1)!.end as number) > paren) return false;
			if (!COMMENT_RUN.test(text.slice(first, paren))) return false;
			// exactly the run: every root comment with only whitespace and comments between
			// it and the `(` — the whole run ahead of the paren, so a dropped head shows
			const root_comments = (ctx.canonical_root as { comments?: unknown } | null)?.comments;
			if (!Array.isArray(root_comments)) return false;
			const run = distinct_comment_spans(root_comments)
				.map((copies) => copies[0]!)
				.filter(
					(c) =>
						typeof c.start === 'number' &&
						(c.end as number) <= paren &&
						COMMENT_RUN.test(text.slice(c.start, paren))
				);
			if (
				run.length !== comments.length ||
				!run.every((c, i) => c.start === comments[i]!.start && c.end === comments[i]!.end)
			) {
				return false;
			}
			const attached = attached_comment_spans(ctx.canonical_root);
			return comments.every((c) => !attached.has(`${c!.start}:${c!.end}`));
		}
	},
	{
		// acorn-typescript ends a typed RestElement at the binding, excluding the
		// type annotation (`(...args: Array<any>)` → end after `args`) — inconsistent
		// with its own Identifier params, and with babel/TS-ESLint, which include
		// the annotation like tsv does. Gated on both values: ours is the annotation's
		// end, canonical's the binding's.
		name: 'rest_param_type_end',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — Rest param type-annotation end',
		matches: (entry, canonical_parent) => {
			if (entry.kind !== 'value_mismatch' || !/(^|\.)end$/.test(entry.path)) return false;
			const owner = canonical_parent as {
				type?: unknown;
				argument?: { end?: unknown } | null;
				typeAnnotation?: { end?: unknown } | null;
			} | null;
			return (
				owner?.type === 'RestElement' &&
				typeof owner.typeAnnotation?.end === 'number' &&
				entry.ours === owner.typeAnnotation.end &&
				entry.canonical === owner.argument?.end
			);
		}
	},
	{
		// `static` newline `static` in a class body: tsc reads modifier + member (a
		// static field named `static`); acorn ASI-splits every bare `static` into its
		// own value-less field. tsv follows tsc. Scoped to the class body holding
		// an ASI-split `static` (`ladder_start`) — a non-static field acorn ended at its
		// key, with no `;`, and a line break before the next member, so neither a written
		// `static;` nor a written `static static⏎` is a ladder — and within it to the
		// member list from that field on: tsv's members pair up where acorn's do not, so
		// every later index is renumbered, while a member BEFORE the ladder pairs with its
		// own and must still agree.
		name: 'static_member_ladder',
		conformance_section: 'TypeScript Parser Corrections (corpus-enforced) — static member ladder',
		matches: (entry, _canonical_parent, ctx) => {
			const segments = entry.path.split('.');
			// innermost class body first; an outer ladder renumbers a nested class too
			for (let i = segments.length - 1; i > 0; i--) {
				const member = /^body(?:\[(\d+)\])?$/.exec(segments[i]!);
				if (!member) continue;
				const node = get_at_path(ctx.canonical_root, segments.slice(0, i).join('.')) as {
					type?: unknown;
					body?: unknown;
				} | null;
				if (node?.type !== 'ClassBody' || !Array.isArray(node.body)) continue;
				const start = ladder_start(node.body, offset_text(ctx));
				if (start < 0) continue;
				if (member[1] === undefined) {
					// the member list itself: only its length moves
					if (i === segments.length - 1 && entry.kind === 'length_mismatch') return true;
					continue;
				}
				if (Number(member[1]) >= start) return true;
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
		// shape uniformly. Gated on the line break the shape depends on, and on tsv's
		// shape being canonical's instantiation UNWRAPPED: each field of our superClass
		// is that field of its `expression`, and our `superTypeParameters` is its
		// `typeArguments` — so a wrong superclass or wrong type arguments stay
		// undocumented.
		name: 'extends_instantiation_linebreak',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — extends instantiation line-break shape',
		matches: (entry, _canonical_parent, ctx) => {
			const m = /^(.*)\.(?:superClass(?:\.([^.[]+))?|superTypeParameters)$/.exec(entry.path);
			if (!m) return false;
			const cls = get_at_path(ctx.canonical_root, m[1]!) as {
				superClass?: Record<string, unknown> | null;
			} | null;
			const instantiation = cls?.superClass;
			if (instantiation?.type !== 'TSInstantiationExpression') return false;
			if (typeof instantiation.end !== 'number') return false;
			if (!line_break_follows(offset_text(ctx), instantiation.end)) return false;
			if (entry.path.endsWith('.superTypeParameters')) {
				return (
					entry.kind === 'missing_canonical' && json_equal(entry.ours, instantiation.typeArguments)
				);
			}
			const field = m[2];
			if (field === undefined) return false;
			if (entry.kind === 'missing_ours') return field === 'expression' || field === 'typeArguments';
			const unwrapped = instantiation.expression as Record<string, unknown> | null | undefined;
			return unwrapped != null && json_equal(entry.ours, unwrapped[field]);
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
		// and with babel, which start at the `(` like tsv does. Gated on both values:
		// ours is the `(` the `@` opens (not a paren nested inside it), canonical's is
		// past it with only whitespace and further opening parens between — and the node
		// is one built ON the parens, ending past their `)`, never the expression inside.
		name: 'decorator_paren_subscript_start',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — Parenthesized decorator subscript start',
		matches: (entry, canonical_parent, ctx) => {
			if (entry.kind !== 'value_mismatch') return false;
			const m = entry.path.match(
				/^(.*\.decorators\[\d+\]\.expression)(?:\.(?:callee|object|expression))*\.start$/
			);
			if (!m) return false;
			const expr = get_at_path(ctx.canonical_root, m[1]!) as { type?: unknown } | null;
			if (expr?.type !== 'CallExpression' && expr?.type !== 'MemberExpression') return false;
			const { ours, canonical } = entry;
			if (typeof ours !== 'number' || typeof canonical !== 'number' || canonical <= ours) {
				return false;
			}
			const text = offset_text(ctx);
			// ours is the OUTERMOST paren — the one the `@` opens — and canonical past it
			if (
				text[ours] !== '(' ||
				!/@\s*$/.test(text.slice(0, ours)) ||
				!/^[\s(]*$/.test(text.slice(ours + 1, canonical))
			) {
				return false;
			}
			const close = matching_close_paren(text, ours);
			const end = (canonical_parent as { end?: unknown } | null)?.end;
			return close !== -1 && typeof end === 'number' && end > close;
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
		// that arm cannot be scoped per index — it is scoped to the stylesheet's own
		// `comments` array and, within it, to the indices from the first comment at or
		// past the first garbage declaration on: a comment AHEAD of the garbage pairs
		// with its own and must still agree. It is scoped by
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
			// A root `comments` is the stylesheet's only in a CSS document; a Svelte root's is
			// the JS comment list, which no CSS garbage can renumber.
			const in_sheet = entry.path.startsWith('css.');
			if (!in_sheet && ctx.language !== 'css') return false;
			if (entry.kind === 'length_mismatch' && Number(entry.ours) <= Number(entry.canonical)) {
				return false;
			}
			if (entry.kind === 'missing_ours' && !/\[\d+\]\.position$/.test(entry.path)) return false;
			const sheet = (in_sheet ? get_at_path(ctx.canonical_root, 'css') : ctx.canonical_root) as {
				comments?: unknown;
			} | null;
			const first = first_garbage_declaration_start(sheet);
			if (first === null || !Array.isArray(sheet?.comments)) return false;
			const index = /^(?:css\.)?comments\[(\d+)\]/.exec(entry.path)?.[1];
			if (index === undefined) return true;
			const ahead = sheet.comments.filter(
				(c: CommentLike | null) => typeof c?.start === 'number' && c.start < first
			).length;
			return Number(index) >= ahead;
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
					/^\s*\(\s*\(/.test(offset_text(ctx).slice(node.typeParameters.end))
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

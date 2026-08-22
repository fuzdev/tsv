/**
 * Diagnostic: prove the `no-locations` wire is losslessly reconstructible.
 *
 * The span-only variant drops per-node `loc` (and Svelte `name_loc`) because
 * line/column is a pure function of a node's `start`/`end` (UTF-16 offsets) plus
 * the source. This is the reference reconstruction a consumer would use: it
 * parses each file the full (loc-bearing) way as the oracle, rebuilds `loc` from
 * offsets + source, and asserts they match — so a consumer holding only the
 * no-locations wire can recover exact acorn/svelte `loc` on demand.
 *
 * Svelte's `name_loc` gets the same treatment. Its name span isn't a node
 * `start`/`end`, but it is a function of them plus the node type — the tag-name run
 * after `<`, an attribute name at the node start (a shorthand `{x}` naming the
 * identifier inside its braces), a directive's whole head token — so this file
 * derives the span and gates both it and its line/column against the oracle wire.
 *
 * Line rules (mirrored from `tsv_lang::LocationTracker`):
 * - TypeScript / `.ts`: ECMAScript LineTerminators — \n, \r, \r\n (ONE), U+2028,
 *   U+2029 (`new_ecmascript_with_map`).
 * - Svelte / `.svelte`: LF-only, the parser's locate-character convention, applied
 *   to the whole document incl. embedded `<script>`/`{expr}` (`new_with_map`).
 * Column is 0-based UTF-16 code units; line is 1-based. Wire offsets are UTF-16,
 * and a JS string is UTF-16-indexed, so a line-start table built by scanning the
 * JS string is directly comparable — no byte handling.
 *
 * Known Svelte wrinkle (classified, not failed): destructure patterns inside
 * `{#each … as …}` / `{:then}` / `{:catch}` / `{@const}` carry a `+1` column on
 * the pattern's start line (Svelte parses them under a synthetic `(`). That's a
 * deterministic parser quirk, not a pure offset derivation, so a column that is
 * exactly `+1` over the reconstruction on a `.svelte` file is reported as
 * `pattern_quirk`, not a mismatch.
 *
 * Run: deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys \
 *   benches/js/diagnostics/no_locations_parity.ts 2>&1 | tail -30
 */

import { DevReposLoader, group_by_language } from '../lib/corpus.ts';
import { init_implementations } from '../lib/implementations.ts';
import type { Language } from '../lib/types.ts';

type LineRule = 'ecmascript' | 'lf';

/** Line-start offsets (UTF-16 units), rightmost-<=-offset lookup gives the line. */
function build_line_starts(src: string, rule: LineRule): number[] {
	const starts = [0];
	for (let i = 0; i < src.length; i++) {
		const c = src.charCodeAt(i);
		if (c === 0x0a) {
			starts.push(i + 1); // \n
		} else if (rule === 'ecmascript') {
			if (c === 0x0d) {
				if (src.charCodeAt(i + 1) === 0x0a) i++; // \r\n counts as one
				starts.push(i + 1);
			} else if (c === 0x2028 || c === 0x2029) {
				starts.push(i + 1);
			}
		}
	}
	return starts;
}

function loc_at(offset: number, starts: number[]): { line: number; column: number } {
	let lo = 0;
	let hi = starts.length - 1;
	while (lo < hi) {
		const mid = (lo + hi + 1) >> 1;
		if (starts[mid] <= offset) lo = mid;
		else hi = mid - 1;
	}
	return { line: lo + 1, column: offset - starts[lo] };
}

interface Tally {
	/** Svelte sources refused up front: their two line classes disagree (see `TWO_LINE_CLASSES`). */
	two_line_classes: number;
	/** Svelte trees refused up front: a block binding's `: T` is split from it by a newline. */
	seeded_annotation: number;
	exact: number;
	pattern_quirk: number;
	script_override: number;
	mismatch: number;
	name_loc_exact: number;
	name_loc_mismatch: number;
	name_span_exact: number;
	name_span_mismatch: number;
}

/**
 * A terminator the ECMAScript class counts and the LF class does not — a lone `\r`,
 * U+2028 or U+2029 (`\r\n` is one ECMAScript break holding one LF, so the classes agree
 * over it). A Svelte source holding one carries TWO line counts — acorn's on the nodes it
 * parsed, `locate-character`'s on the rest — and which one a node takes is not a function
 * of its offsets, so the wire is genuinely not reconstructible there. Re-derived rather
 * than imported, like everything else here: this file is the independent oracle.
 */
const TWO_LINE_CLASSES = /\r(?!\n)|[\u2028\u2029]/;

/**
 * The shipped helper's SECOND refusal, re-derived: a Svelte block binding whose `: T` is
 * separated from it by a newline. Svelte reads that annotation with its own acorn parse,
 * entered on a synthetic `_ as ` that OVERWRITES the five bytes behind the colon — so the
 * break is erased before acorn sees it and the annotation's nodes stay on the binding's
 * line, which no single table over the real source produces.
 *
 * Mirrored here for the reason this whole file exists: it is the independent oracle for what
 * the reconstruction can serve, and an oracle scoped differently from the thing it grades
 * reports a divergence that is really a disagreement about the corpus. The discriminator is
 * a leading-whitespace run holding a newline — every acorn-built `TSTypeAnnotation` opens on
 * a token (a `:`, or a function type's `=>`), while only Svelte's block-binding one is
 * anchored at the *binding's* end.
 */
function has_seeded_annotation(node: unknown, source: string): boolean {
	if (!node || typeof node !== 'object') return false;
	if (Array.isArray(node)) return node.some((item) => has_seeded_annotation(item, source));
	const record = node as Record<string, unknown>;
	if (record.type === 'TSTypeAnnotation' && typeof record.start === 'number') {
		for (let i = record.start; i < source.length && /\s/.test(source[i]); i++) {
			if (source[i] === '\n') return true;
		}
	}
	return Object.keys(record).some(
		(key) => key !== 'loc' && key !== 'name_loc' && has_seeded_annotation(record[key], source)
	);
}

// The Svelte name span is derivable from the node's own start/end + type, so the
// no-locations wire keeps `name_loc` recoverable too. Re-derived here rather than
// imported from the shipped helper (crates/tsv_wasm/npm/locations.js) — this file
// is the independent oracle that gates it.

/** Node types whose name is the tag-name run right after `<`. */
const ELEMENT_NAME_TYPES = new Set([
	'RegularElement',
	'Component',
	'SvelteHead',
	'SvelteWindow',
	'SvelteBody',
	'SvelteDocument',
	'SvelteElement',
	'SvelteComponent',
	'SvelteSelf',
	'SlotElement',
	'SvelteFragment',
	'SvelteBoundary',
	'TitleElement'
]);

/** Node types whose name span is the whole directive head (`on:click|preventDefault`). */
const DIRECTIVE_NAME_TYPES = new Set([
	'OnDirective',
	'BindDirective',
	'ClassDirective',
	'StyleDirective',
	'UseDirective',
	'TransitionDirective', // `in:`/`out:` too — Svelte has no In/OutDirective type
	'AnimateDirective',
	'LetDirective'
]);

/**
 * The chars that end an attribute/directive name run — tsv's parse of Svelte's
 * `read_tag` (`/[\s=/>"']/`), ASCII whitespace only.
 */
const NAME_TERMINATORS = ' \t\n\r\v\f=/>"\'';

/** The `[start, end]` offsets a node's `name_loc` covers, or null if it carries none. */
function name_span_of(node: Record<string, unknown>, source: string): [number, number] | null {
	const { type, name, start, end } = node as {
		type: string;
		name?: string;
		start?: number;
		end?: number;
	};
	if (typeof name !== 'string' || typeof start !== 'number' || typeof end !== 'number') return null;
	if (ELEMENT_NAME_TYPES.has(type)) return [start + 1, start + 1 + name.length];
	if (DIRECTIVE_NAME_TYPES.has(type)) {
		let head_end = start;
		while (head_end < end && !NAME_TERMINATORS.includes(source[head_end])) head_end++;
		return [start, head_end];
	}
	if (type === 'Attribute') {
		// a shorthand `{x}` names the identifier inside the braces, so a padded `{ x }`
		// excludes the padding — unlike a `<script>` attribute, whose literal name can
		// itself be braced (`<script {x}>`, name `{x}`)
		if (source[start] === '{' && !source.startsWith(name, start)) {
			const name_start = source.indexOf(name, start + 1);
			return name_start < 0 || name_start >= end ? null : [name_start, name_start + name.length];
		}
		return [start, start + name.length];
	}
	return null;
}

function check_node(
	node: Record<string, unknown>,
	starts: number[],
	is_svelte: boolean,
	source: string,
	t: Tally
): void {
	const loc = node.loc as
		| { start?: { line: number; column: number }; end?: { line: number; column: number } }
		| undefined;
	if (loc?.start && typeof node.start === 'number') {
		// A Svelte `<script>`/`<style>` `Program` loc is deliberately the *tag*
		// position, not the content offset (Svelte's read_script override) — a
		// documented quirk, derivable from source but not from start/end alone.
		if (is_svelte && node.type === 'Program') {
			t.script_override++;
		} else {
			const got = loc_at(node.start, starts);
			const want = loc.start;
			if (got.line === want.line && got.column === want.column) {
				t.exact++;
			} else if (is_svelte && got.line === want.line && got.column + 1 === want.column) {
				// Svelte destructure-pattern synthetic-`(` column shift (+1).
				t.pattern_quirk++;
			} else {
				t.mismatch++;
				if (t.mismatch <= 5) {
					console.error(
						`  loc mismatch ${node.type as string} @${node.start}: got ${got.line}:${got.column} want ${want.line}:${want.column}`
					);
				}
			}
		}
	}
	// Svelte name_loc: its `character` sub-field is the name offset; line/column
	// must reconstruct from it exactly (the spine carries no pattern quirk).
	const nl = node.name_loc as
		| {
				start?: { line: number; column: number; character: number };
				end?: { line: number; column: number; character: number };
		  }
		| undefined;
	if (nl?.start && nl.end) {
		const got = loc_at(nl.start.character, starts);
		if (got.line === nl.start.line && got.column === nl.start.column) t.name_loc_exact++;
		else t.name_loc_mismatch++;
		// The name span itself — the part a no-loc consumer has to derive, since the
		// wire drops `name_loc` whole. Per-type rule (tag name after `<`, attribute
		// name at the node, directive head run), gated like `loc`.
		const span = name_span_of(node, source);
		if (span && span[0] === nl.start.character && span[1] === nl.end.character) {
			t.name_span_exact++;
		} else {
			t.name_span_mismatch++;
			if (t.name_span_mismatch <= 5) {
				console.error(
					`  name span mismatch ${node.type as string} @${node.start}: got ${span ? `[${span[0]},${span[1]}]` : 'null'} want [${nl.start.character},${nl.end.character}]`
				);
			}
		}
	}
}

function walk(
	value: unknown,
	starts: number[],
	is_svelte: boolean,
	source: string,
	t: Tally
): void {
	if (Array.isArray(value)) {
		for (const v of value) walk(v, starts, is_svelte, source, t);
	} else if (value && typeof value === 'object') {
		check_node(value as Record<string, unknown>, starts, is_svelte, source, t);
		for (const v of Object.values(value)) walk(v, starts, is_svelte, source, t);
	}
}

const impls = await init_implementations({ logger: (m) => console.error(m) });
const native = impls.native;

const files = await new DevReposLoader('gates').load((m) => console.error(m));
const by_lang = group_by_language(files);

let any_mismatch = false;
for (const language of ['typescript', 'svelte'] as Language[]) {
	const rule: LineRule = language === 'svelte' ? 'lf' : 'ecmascript';
	const is_svelte = language === 'svelte';
	const t: Tally = {
		two_line_classes: 0,
		seeded_annotation: 0,
		exact: 0,
		pattern_quirk: 0,
		script_override: 0,
		mismatch: 0,
		name_loc_exact: 0,
		name_loc_mismatch: 0,
		name_span_exact: 0,
		name_span_mismatch: 0
	};
	let checked = 0;
	for (const f of by_lang[language] ?? []) {
		// Not a mismatch and not a quirk — a source no offset-keyed reconstruction can
		// serve, which the shipped helper refuses outright. Counted so a corpus that grows
		// one says so instead of silently shrinking the sample.
		if (is_svelte && TWO_LINE_CLASSES.test(f.content)) {
			t.two_line_classes++;
			continue;
		}
		let full: unknown;
		try {
			full = native.parse(f.content, language);
			// Sanity: the no-locations wire really drops loc (the consumer's input).
			const noloc = JSON.stringify(native.parse_no_locations(f.content, language));
			// Look for the loc OBJECT (`"loc":{`), not the substring `"loc"` — the
			// latter matches source identifiers/strings/keys named `loc`.
			if (noloc.includes('"loc":{') || (is_svelte && noloc.includes('"name_loc":{'))) {
				console.error(`  ${f.path}: no-locations wire still carries a loc/name_loc object!`);
				any_mismatch = true;
			}
		} catch {
			continue; // skip files the parser rejects
		}
		// The second refusal, which needs the TREE rather than the source — so unlike the one
		// above it can only be asked here, after the parse.
		if (is_svelte && has_seeded_annotation(full, f.content)) {
			t.seeded_annotation++;
			continue;
		}
		walk(full, build_line_starts(f.content, rule), is_svelte, f.content, t);
		checked++;
	}
	const loc_total = t.exact + t.pattern_quirk + t.script_override + t.mismatch;
	console.error(
		`\n${language}: ${checked} files, ${loc_total} loc nodes — exact ${t.exact}, pattern_quirk ${t.pattern_quirk}, script_tag_override ${t.script_override}, MISMATCH ${t.mismatch}`
	);
	if (is_svelte) {
		console.error(
			`  name_loc: line/col exact ${t.name_loc_exact}, MISMATCH ${t.name_loc_mismatch}; name span exact ${t.name_span_exact}, MISMATCH ${t.name_span_mismatch}`
		);
		// Reported, not just tallied: these are the counts the shipped helper REFUSES, so a
		// corpus that grows one has to say so here rather than silently shrink `checked`.
		console.error(
			`  refused up front: ${t.two_line_classes} two line classes, ` +
				`${t.seeded_annotation} seeded annotation`
		);
	}
	if (t.mismatch > 0 || t.name_loc_mismatch > 0 || t.name_span_mismatch > 0) any_mismatch = true;
}

if (any_mismatch) {
	console.error('\nFAIL: loc/name_loc not fully reconstructible from offsets + source');
	Deno.exit(1);
}
console.error(
	'\nPASS: every loc reconstructs from start/end + source (pattern-quirk columns classified)'
);

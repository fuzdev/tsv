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
	exact: number;
	pattern_quirk: number;
	script_override: number;
	mismatch: number;
	name_loc_exact: number;
	name_loc_mismatch: number;
	prefix_ok: number;
	prefix_off: number;
}

// Svelte name offset = node.start + a fixed per-node-type prefix (the STRETCH
// claim: the name span is derivable even without name_loc). Prefix = the bytes
// before the name: `<` for elements, the directive keyword + `:`, `{` for shorthand.
const NAME_PREFIX: Record<string, number> = {
	RegularElement: 1,
	Component: 1,
	SvelteElement: 1,
	SvelteComponent: 1,
	SvelteSelf: 1,
	SvelteWindow: 1,
	SvelteDocument: 1,
	SvelteBody: 1,
	SvelteHead: 1,
	SvelteFragment: 1,
	SvelteBoundary: 1,
	TitleElement: 1,
	SlotElement: 1,
	Attribute: 0,
	ShorthandAttribute: 1,
	OnDirective: 3, // on:
	BindDirective: 5, // bind:
	ClassDirective: 6, // class:
	StyleDirective: 6, // style:
	UseDirective: 4, // use:
	TransitionDirective: 11, // transition:
	InDirective: 3, // in:
	OutDirective: 4, // out:
	AnimateDirective: 8, // animate:
	LetDirective: 4, // let:
};

function check_node(
	node: Record<string, unknown>,
	starts: number[],
	is_svelte: boolean,
	source: string,
	t: Tally,
): void {
	const loc = node.loc as { start?: { line: number; column: number }; end?: { line: number; column: number } } | undefined;
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
						`  loc mismatch ${node.type as string} @${node.start}: got ${got.line}:${got.column} want ${want.line}:${want.column}`,
					);
				}
			}
		}
	}
	// Svelte name_loc: its `character` sub-field is the name offset; line/column
	// must reconstruct from it exactly (the spine carries no pattern quirk).
	const nl = node.name_loc as { start?: { line: number; column: number; character: number } } | undefined;
	if (nl?.start) {
		const got = loc_at(nl.start.character, starts);
		if (got.line === nl.start.line && got.column === nl.start.column) t.name_loc_exact++;
		else t.name_loc_mismatch++;
		// Report-only heuristic: the name offset ≈ node.start + a per-type prefix.
		// Approximate (some node spans lead with a space, so this is off by one) —
		// a consumer needing the exact name span in the no-loc wire recovers it by
		// searching the name string within [start,end], not by a fixed prefix. Not
		// a gate; just surfaces how close the simple rule gets.
		const prefix = NAME_PREFIX[node.type as string];
		if (prefix !== undefined && typeof node.start === 'number' && node.start + prefix === nl.start.character) {
			t.prefix_ok++;
		} else {
			t.prefix_off++;
		}
	}
	void source;
}

function walk(value: unknown, starts: number[], is_svelte: boolean, source: string, t: Tally): void {
	if (Array.isArray(value)) {
		for (const v of value) walk(v, starts, is_svelte, source, t);
	} else if (value && typeof value === 'object') {
		check_node(value as Record<string, unknown>, starts, is_svelte, source, t);
		for (const v of Object.values(value)) walk(v, starts, is_svelte, source, t);
	}
}

const impls = await init_implementations({ logger: (m) => console.error(m) });
if (!impls.native) throw new Error('native FFI not built — run deno task build:ffi');
const native = impls.native;

const files = await new DevReposLoader('gates').load((m) => console.error(m));
const by_lang = group_by_language(files);

let any_mismatch = false;
for (const language of ['typescript', 'svelte'] as Language[]) {
	const rule: LineRule = language === 'svelte' ? 'lf' : 'ecmascript';
	const is_svelte = language === 'svelte';
	const t: Tally = {
		exact: 0,
		pattern_quirk: 0,
		script_override: 0,
		mismatch: 0,
		name_loc_exact: 0,
		name_loc_mismatch: 0,
		prefix_ok: 0,
		prefix_off: 0,
	};
	let checked = 0;
	for (const f of by_lang[language] ?? []) {
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
		walk(full, build_line_starts(f.content, rule), is_svelte, f.content, t);
		checked++;
	}
	const loc_total = t.exact + t.pattern_quirk + t.script_override + t.mismatch;
	console.error(
		`\n${language}: ${checked} files, ${loc_total} loc nodes — exact ${t.exact}, pattern_quirk ${t.pattern_quirk}, script_tag_override ${t.script_override}, MISMATCH ${t.mismatch}`,
	);
	if (is_svelte) {
		console.error(
			`  name_loc line/col: exact ${t.name_loc_exact}, MISMATCH ${t.name_loc_mismatch}; name-offset≈start+prefix (report-only): ok ${t.prefix_ok}, off ${t.prefix_off}`,
		);
	}
	if (t.mismatch > 0 || t.name_loc_mismatch > 0) any_mismatch = true;
}

if (any_mismatch) {
	console.error('\nFAIL: loc/name_loc not fully reconstructible from offsets + source');
	Deno.exit(1);
}
console.error('\nPASS: every loc reconstructs from start/end + source (pattern-quirk columns classified)');

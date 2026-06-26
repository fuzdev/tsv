/**
 * Comment-duplication completeness scan (diagnostic; ad-hoc, not wired into `deno task`).
 *
 * Cross-checks the 2026-06-17 comment-dedup pass: tsv now emits each comment ONCE
 * in the public AST for every construct, correcting acorn-typescript's
 * backtrack-reparse duplication instead of replicating it. This scan looks for
 * anything that pass missed.
 *
 * A "duplicate" here is acorn's signature, not Svelte's legitimate model: the same
 * comment span (start,end) appearing **>=2 times within a single array** — the root
 * `comments` array, or one node's `leadingComments` / `trailingComments`. The
 * legitimate (non-bug) shape is 1x in root `comments` + 1x in exactly one attachment
 * array; per-array grouping keeps that from false-flagging.
 *
 * Two independent oracles, per the prompt:
 *   (1) live canonical parse over each fixture input (svelte/compiler) — the ground
 *       truth of where acorn duplicates today.
 *   (2) the committed expected JSON (expected.json / expected_svelte.json / expected_ours.json).
 *
 * Run:
 *   deno run --allow-read --allow-env --allow-net --allow-sys \
 *     --config benches/js/deno.json benches/js/diagnostics/comment_dup_scan.ts [--json] [--verbose]
 *
 * Only `.svelte` fixtures can carry comment dups: `.ts`/`.svelte.ts` route to
 * acorn-typescript with NO onComment (the sidecar never sets it), so their canonical
 * output has no comments at all, and `.css` is unaffected by the TS backtrack bug.
 * The scan still walks every fixture's JSON to prove that empirically.
 */

import { walk } from '@std/fs';
import { parse as svelte_parse } from 'svelte/compiler';
import * as acorn from 'acorn';
import { tsPlugin } from '@sveltejs/acorn-typescript';

// deno-lint-ignore no-explicit-any
const ParserWithTs = acorn.Parser.extend(tsPlugin() as any);

const FIXTURES_ROOT = `${import.meta.dirname}/../../../tests/fixtures`;

const args = new Set(Deno.args);
const JSON_OUT = args.has('--json');
const VERBOSE = args.has('--verbose');

// ── dup detection ────────────────────────────────────────────────────────────

interface Dup {
	array_path: string; // path of the containing array (index stripped)
	span: string; // "start,end"
	count: number; // occurrences in that single array
	value: string;
	in_root_comments: boolean; // root `comments` array vs an attachment array
}

// deno-lint-ignore no-explicit-any
function is_comment_node(n: any): boolean {
	return (
		n != null &&
		typeof n === 'object' &&
		(n.type === 'Block' || n.type === 'Line') &&
		typeof n.start === 'number' &&
		typeof n.end === 'number'
	);
}

/** Find spans that occur >=2 times within a single comment array. */
function find_dups(root: unknown): Dup[] {
	// container_array_path -> span -> {count, value}
	const groups = new Map<string, Map<string, { count: number; value: string }>>();

	// deno-lint-ignore no-explicit-any
	function visit(node: any, path: string, container_array_path: string | null) {
		if (Array.isArray(node)) {
			for (let i = 0; i < node.length; i++) visit(node[i], `${path}[${i}]`, path);
			return;
		}
		if (node == null || typeof node !== 'object') return;
		if (is_comment_node(node) && container_array_path !== null) {
			const key = `${node.start},${node.end}`;
			let g = groups.get(container_array_path);
			if (!g) {
				g = new Map();
				groups.set(container_array_path, g);
			}
			const cur = g.get(key) ?? { count: 0, value: String(node.value ?? '') };
			cur.count++;
			g.set(key, cur);
		}
		// recursing into object properties resets the array context to null
		for (const k of Object.keys(node)) {
			visit(node[k], path ? `${path}.${k}` : k, null);
		}
	}

	visit(root, '', null);

	const dups: Dup[] = [];
	for (const [array_path, g] of groups) {
		for (const [span, info] of g) {
			if (info.count >= 2) {
				dups.push({
					array_path,
					span,
					count: info.count,
					value: info.value,
					in_root_comments: array_path === 'comments' || array_path.endsWith('.comments'),
				});
			}
		}
	}
	return dups;
}

function span_set(dups: Dup[]): Set<string> {
	// identity of a dup = which array + which span (dedupe for set-compare)
	return new Set(dups.map((d) => `${d.array_path}@${d.span}`));
}

// ── construct inference ──────────────────────────────────────────────────────

/** Map a fixture path to one of the deep-dive's mechanisms (or a broader bucket). */
function construct_of(relpath: string): string {
	const p = relpath;
	if (/type_assertion/.test(p)) return 'type assertion <T> (tryParse)';
	// method/abstract/declare return-type -> `;` trailing comment (also interface/type-literal
	// method, call/construct signatures) — one tryParse backtrack family, consolidated per context
	if (/trailing_semicolon|method_trailing/.test(p)) return 'method/abstract return-type → ; (tryParse)';
	if (/arrow/.test(p)) return 'arrow return-type / pre-arrow (tryParse)';
	if (/index_signature/.test(p)) return 'index signature (tsLookAhead)';
	if (/function_type/.test(p)) return 'function type param (tsLookAhead)';
	if (/mapped/.test(p)) return 'mapped type (tsLookAhead)';
	if (/computed_key/.test(p)) return 'computed key (tsLookAhead)';
	if (/call_type_arg|generics/.test(p)) return 'call type-arg <...> (tryParse)';
	if (/type_literal|type_members|types\/comments/.test(p)) return 'type-literal member (speculative layer)';
	if (/prettier_ignore_members/.test(p)) return 'type-literal member (speculative layer)';
	if (/union_/.test(p)) return 'type-literal member (speculative layer)';
	return 'other';
}

function suffix_of(dir: string): 'svelte_prettier' | 'svelte' | 'prettier' | 'plain' {
	if (dir.endsWith('_svelte_prettier_divergence')) return 'svelte_prettier';
	if (dir.endsWith('_svelte_divergence')) return 'svelte';
	if (dir.endsWith('_prettier_divergence')) return 'prettier';
	return 'plain';
}

// ── fixture model ────────────────────────────────────────────────────────────

interface FixtureScan {
	rel: string; // dir relative to tests/fixtures
	ext: string; // input.svelte / input.ts / input.svelte.ts / input.css
	suffix: ReturnType<typeof suffix_of>;
	is_svelte_div: boolean;
	construct: string;
	// committed JSON dups
	ours_file: string | null; // tsv's output JSON
	canon_file: string | null; // canonical JSON
	ours_dups: Dup[];
	canon_dups: Dup[];
	// live canonical (svelte only)
	live_dups: Dup[] | null; // null => not live-parsed (non-svelte) or parse error
	live_error: string | null;
}

async function read_json(path: string): Promise<unknown | null> {
	try {
		return JSON.parse(await Deno.readTextFile(path));
	} catch {
		return null;
	}
}

async function exists(path: string): Promise<boolean> {
	try {
		await Deno.stat(path);
		return true;
	} catch {
		return false;
	}
}

function input_name(ext: string): string {
	return `input.${ext}`;
}

// ── main ─────────────────────────────────────────────────────────────────────

const fixtures: FixtureScan[] = [];

const INPUT_NAMES = new Set(['input.svelte', 'input.svelte.ts', 'input.ts', 'input.css']);

for await (
	const entry of walk(FIXTURES_ROOT, { includeDirs: false, includeFiles: true })
) {
	if (!INPUT_NAMES.has(entry.name)) continue;
	const dir = entry.path.slice(0, entry.path.length - entry.name.length - 1);
	// derive rel from the marker — FIXTURES_ROOT carries unresolved ../.. that walk normalizes away
	const marker = '/tests/fixtures/';
	const rel = dir.slice(dir.indexOf(marker) + marker.length);
	const ext = entry.name.replace(/^input\./, '');
	const suffix = suffix_of(dir.replace(/.*\//, ''));
	const is_svelte_div = suffix === 'svelte' || suffix === 'svelte_prettier';

	const has_ours = await exists(`${dir}/expected_ours.json`);
	const has_svelte = await exists(`${dir}/expected_svelte.json`);
	const has_plain = await exists(`${dir}/expected.json`);

	// tsv's output JSON: expected_ours.json when a divergence splits them, else expected.json
	const ours_file = has_ours ? 'expected_ours.json' : has_plain ? 'expected.json' : null;
	// canonical JSON: expected_svelte.json when present, else expected.json
	const canon_file = has_svelte ? 'expected_svelte.json' : has_plain ? 'expected.json' : null;

	const ours_json = ours_file ? await read_json(`${dir}/${ours_file}`) : null;
	const canon_json = canon_file ? await read_json(`${dir}/${canon_file}`) : null;

	const ours_dups = ours_json ? find_dups(ours_json) : [];
	const canon_dups = canon_json ? find_dups(canon_json) : [];

	// live canonical parse — only the svelte path surfaces acorn's onComment duplication
	let live_dups: Dup[] | null = null;
	let live_error: string | null = null;
	if (ext === 'svelte') {
		try {
			const source = await Deno.readTextFile(`${dir}/${input_name(ext)}`);
			const ast = svelte_parse(source, { modern: true });
			live_dups = find_dups(ast);
		} catch (err) {
			live_error = err instanceof Error ? err.message : String(err);
		}
	}

	fixtures.push({
		rel,
		ext,
		suffix,
		is_svelte_div,
		construct: construct_of(rel),
		ours_file,
		canon_file,
		ours_dups,
		canon_dups,
		live_dups,
		live_error,
	});
}

fixtures.sort((a, b) => a.rel.localeCompare(b.rel));

// ── classify ─────────────────────────────────────────────────────────────────

// RED #1: tsv's own output still carries a dup anywhere
const ours_dup = fixtures.filter((f) => f.ours_dups.length > 0);

// RED #2: a non-svelte-divergence fixture whose canonical JSON has a dup
//         (a duplicating input not captured as a _svelte_divergence)
const plain_canon_dup = fixtures.filter((f) => !f.is_svelte_div && f.canon_dups.length > 0);

// GREEN: svelte-divergence correctly capturing a dup (canon has dup, ours single)
const captured = fixtures.filter(
	(f) => f.is_svelte_div && f.canon_dups.length > 0 && f.ours_dups.length === 0,
);

// YELLOW: svelte-divergence with NO dup in canonical — suffix justified by some other
//         reason (not comment duplication), or a mislabel worth eyeballing
const div_no_dup = fixtures.filter((f) => f.is_svelte_div && f.canon_dups.length === 0);

// RED #3 (key completeness signal via the live oracle): input live-parses to a dup but
//         the fixture is NOT a svelte-divergence (plain or _prettier-only)
const live_dup_uncaptured = fixtures.filter(
	(f) => f.live_dups && f.live_dups.length > 0 && !f.is_svelte_div,
);

// RED #4: live canonical dup-set disagrees with the committed canonical JSON (stale fixture)
const live_vs_committed_stale = fixtures.filter((f) => {
	if (!f.live_dups) return false;
	// compare against whichever committed file represents canonical
	const a = span_set(f.live_dups);
	const b = span_set(f.canon_dups);
	if (a.size !== b.size) return true;
	for (const x of a) if (!b.has(x)) return true;
	return false;
});

// A live parse error is benign for THIS scan whenever the committed canonical carries
// no comment dup — there's nothing a successful parse could reveal that we're missing.
// These are the "tsv parses, Svelte rejects" divergences (using/v-flag-regex/CSS-
// namespace/...) whose expected_svelte.json records Svelte's parse ERROR, not a tree
// (0 comment nodes). Only a live error where committed canonical HAS a dup would be a
// real contradiction (stale fixture vs the pinned parser).
const live_errors_benign = fixtures.filter((f) => f.live_error && f.canon_dups.length === 0);
const live_errors_concerning = fixtures.filter((f) => f.live_error && f.canon_dups.length > 0);

// ── construct rollups ────────────────────────────────────────────────────────

function by_construct(fs: FixtureScan[]): Map<string, FixtureScan[]> {
	const m = new Map<string, FixtureScan[]>();
	for (const f of fs) {
		const k = f.construct;
		(m.get(k) ?? m.set(k, []).get(k)!).push(f);
	}
	return m;
}

const captured_by_construct = by_construct(captured);

// The deep-dive's duplicating mechanisms — cross-ref which have >=1 captured fixture.
// (method + abstract-method return-type→; are one consolidated family here.)
const KNOWN_MECHANISMS = [
	'computed key (tsLookAhead)',
	'index signature (tsLookAhead)',
	'function type param (tsLookAhead)',
	'mapped type (tsLookAhead)',
	'type assertion <T> (tryParse)',
	'arrow return-type / pre-arrow (tryParse)',
	'method/abstract return-type → ; (tryParse)',
];

// ── detector self-test (deep-dive's canonical minimal repros) ────────────────

interface SelfTest {
	label: string;
	source: string;
	dups: number;
	ok: boolean;
}
const SELF_TESTS: { label: string; ts: string }[] = [
	{ label: 'predicate / tsLookAhead (computed key)', ts: 'class C { [ /* c */ foo ]() {} }' },
	{ label: 'backtrack / tryParse (arrow return-type)', ts: 'const f = (x): T /* c */ => 0;' },
];
const self_tests: SelfTest[] = [];
for (const t of SELF_TESTS) {
	try {
		const ast = svelte_parse(`<script lang="ts">\n${t.ts}\n</script>`, { modern: true });
		const d = find_dups(ast);
		self_tests.push({ label: t.label, source: t.ts, dups: d.length, ok: d.length > 0 });
	} catch (err) {
		self_tests.push({
			label: t.label,
			source: t.ts,
			dups: -1,
			ok: false,
		});
		console.error(`self-test parse error: ${err instanceof Error ? err.message : String(err)}`);
	}
}

// ── output ───────────────────────────────────────────────────────────────────

if (JSON_OUT) {
	console.log(
		JSON.stringify(
			{
				totals: {
					fixtures: fixtures.length,
					by_ext: Object.fromEntries(
						[...new Set(fixtures.map((f) => f.ext))].map((
							e,
						) => [e, fixtures.filter((f) => f.ext === e).length]),
					),
					captured_divergences: captured.length,
				},
				red: {
					ours_dup: ours_dup.map((f) => ({ rel: f.rel, file: f.ours_file, dups: f.ours_dups })),
					plain_canon_dup: plain_canon_dup.map((f) => ({ rel: f.rel, dups: f.canon_dups })),
					live_dup_uncaptured: live_dup_uncaptured.map((f) => ({ rel: f.rel, dups: f.live_dups })),
					live_vs_committed_stale: live_vs_committed_stale.map((f) => ({
						rel: f.rel,
						live: f.live_dups,
						committed: f.canon_dups,
					})),
					live_errors_concerning: live_errors_concerning.map((f) => ({
						rel: f.rel,
						error: f.live_error,
					})),
				},
				live_errors_benign: live_errors_benign.map((f) => ({
					rel: f.rel,
					error: f.live_error,
				})),
				captured: captured.map((f) => ({
					rel: f.rel,
					construct: f.construct,
					suffix: f.suffix,
					canon_dup_count: f.canon_dups.length,
				})),
				div_no_dup: div_no_dup.map((f) => ({ rel: f.rel, suffix: f.suffix, construct: f.construct })),
				captured_by_construct: Object.fromEntries(
					[...captured_by_construct].map(([k, v]) => [k, v.map((f) => f.rel)]),
				),
				self_tests,
			},
			null,
			2,
		),
	);
} else {
	const line = '─'.repeat(78);
	const p = (s = '') => console.log(s);

	p(line);
	p('COMMENT-DUPLICATION COMPLETENESS SCAN');
	p(line);
	p(`fixtures scanned:        ${fixtures.length}`);
	const exts = [...new Set(fixtures.map((f) => f.ext))].sort();
	for (const e of exts) p(`  input.${e.padEnd(12)} ${fixtures.filter((f) => f.ext === e).length}`);
	p();
	p(`captured divergences:    ${captured.length}  (canonical dup, tsv single — the healthy set)`);
	p(`svelte-div without dup:  ${div_no_dup.length}  (suffix justified by a non-dup reason — inspect)`);
	p();

	p(line);
	p('RED — must be empty');
	p(line);
	const red = (label: string, fs: FixtureScan[], detail: (f: FixtureScan) => string) => {
		const mark = fs.length === 0 ? '✅' : '❌';
		p(`${mark} ${label}: ${fs.length}`);
		for (const f of fs) p(`      ${detail(f)}`);
	};
	red(
		'tsv output JSON still carrying a dup (ours_dup)',
		ours_dup,
		(f) => `${f.rel}  [${f.ours_file}]  ${f.ours_dups.map((d) => d.array_path + '@' + d.span).join(', ')}`,
	);
	red(
		'non-svelte-divergence fixture w/ canonical dup (plain_canon_dup)',
		plain_canon_dup,
		(f) => `${f.rel}  [${f.suffix}]  ${f.canon_dups.map((d) => d.array_path + '@' + d.span).join(', ')}`,
	);
	red(
		'live canonical dup but fixture NOT a svelte-divergence (live_dup_uncaptured)',
		live_dup_uncaptured,
		(f) => `${f.rel}  [${f.suffix}]`,
	);
	red(
		'live parse disagrees with committed canonical JSON (stale)',
		live_vs_committed_stale,
		(f) => `${f.rel}  live=${f.live_dups!.length} committed=${f.canon_dups.length}`,
	);
	red(
		'live parse error where committed canonical HAS a dup (contradiction)',
		live_errors_concerning,
		(f) => `${f.rel}  ${f.live_error}`,
	);
	p();
	p(
		`note: ${live_errors_benign.length} live parse errors on Svelte-rejects-input divergences ` +
			`(committed canonical records Svelte's error, 0 comment nodes) — expected, not a finding:`,
	);
	for (const f of live_errors_benign) p(`      ${f.rel}`);
	p();

	p(line);
	p('CAPTURED DIVERGENCES — grouped by construct');
	p(line);
	for (const [k, v] of [...captured_by_construct].sort((a, b) => b[1].length - a[1].length)) {
		p(`  ${k}  (${v.length})`);
		if (VERBOSE) {
			for (const f of v) {
				const live = f.live_dups ? `${f.live_dups.length} live` : 'n/a';
				p(`      ${f.rel}  [${f.suffix}]  canon=${f.canon_dups.length} ${live}`);
			}
		}
	}
	p();

	p(line);
	p('DUPLICATING MECHANISMS — coverage cross-ref (each must have >=1 captured fixture)');
	p(line);
	for (const c of KNOWN_MECHANISMS) {
		const hits = captured.filter((f) => f.construct === c).length;
		p(`  ${hits > 0 ? '✅' : '⚠️ '} ${c}: ${hits} captured fixture(s)`);
	}
	const speculative = captured.filter((f) => /type-literal member/.test(f.construct)).length;
	p(`  ℹ️  type-literal member (extra speculative layer): ${speculative} captured fixture(s)`);
	p();

	p(line);
	p('DETECTOR SELF-TEST (deep-dive minimal repros, live svelte parse)');
	p(line);
	for (const t of self_tests) {
		p(`  ${t.ok ? '✅' : '❌'} ${t.label}: ${t.dups} dup-group(s)  «${t.source}»`);
	}
	p();

	if (div_no_dup.length > 0) {
		p(line);
		p('SVELTE-DIVERGENCE FIXTURES WITHOUT A COMMENT DUP (inspect — non-dup justification)');
		p(line);
		for (const f of div_no_dup) p(`  ${f.rel}  [${f.suffix}]  construct=${f.construct}`);
		p();
	}
}

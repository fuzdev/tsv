/**
 * Validates the built WASM artifacts: binary sizes against expected ranges,
 * plus a Deno runtime smoke test of every built bundle.
 *
 * Size bounds are deliberately tight (~±8%) so a legitimate size change
 * fails here and gets acknowledged by updating the constants below — a
 * 100 KB regression (or win) should be visible, not absorbed by slack.
 *
 * The smoke test covers what `scripts/test_npm.ts` (Node) can't:
 * - the npm packages' `index.js` entry running under Deno (the package
 *   README claims zero-config Node/Bun/Deno)
 * - the `pkg/<variant>/deno/` bundles (the format/parse subset bundles feed
 *   nothing else that executes them — benches run only the `all` build)
 *
 * Only validates artifacts that exist (skips unbuilt targets), but fails
 * if nothing was found at all.
 *
 * Usage: deno task validate:artifacts
 */

import { format_size } from './size.ts';

const root = new URL('..', import.meta.url);

let passed = 0;
let failed = 0;
let skipped = 0;

function pass(msg: string): void {
	console.log(`  PASS: ${msg}`);
	passed++;
}

function fail(msg: string): void {
	console.log(`  FAIL: ${msg}`);
	failed++;
}

function skip(msg: string): void {
	console.log(`  SKIP: ${msg}`);
	skipped++;
}

function file_size(path: URL): number | null {
	try {
		return Deno.statSync(path).size;
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;
		return null;
	}
}

// --- WASM binary size checks ---

const VARIANTS = ['format', 'parse', 'all'] as const;
const TARGETS = ['npm', 'deno'] as const;

// Measured 2026-07-04 (two size cuts landed together: the wasm32 feature
// baseline moved to `+simd128,+multivalue` — the multivalue return ABI
// shrinks the pair-return-dense parse path most, ≈−9% parse / ≈−4.4%
// format+all — and talc replaced std's default dlmalloc as the wasm32 global
// allocator, dropping a few more KB per bundle): format 2,178,122 B; parse
// 1,015,388 B; all 2,401,628 B (npm == deno, identical `.wasm`). Bounds
// recentered ±8%.
const BOUNDS = {
	format: { min: 2_005_000, max: 2_350_000 },
	parse: { min: 934_000, max: 1_097_000 },
	all: { min: 2_210_000, max: 2_595_000 },
};

// all = format + parse. `all − format` is the parse feature (parser convert
// path; measured 223,506 B — down from 312,547 B when it still carried the
// comment-island round-trip's `Value` deserialization machinery); `all −
// parse` is the format feature (printers + doc builder, dropped from the
// parse-only build at link time; measured 1,386,240 B, ≈unchanged — the
// gate-health signal). A delta near zero means a feature gate broke.
const DELTAS = {
	format: { min: 209_000, max: 245_000 }, // all − format
	parse: { min: 1_295_000, max: 1_525_000 }, // all − parse
};

console.log('=== WASM binary sizes ===');

const sizes: Partial<Record<`${(typeof VARIANTS)[number]}/${(typeof TARGETS)[number]}`, number>> =
	{};

for (const target of TARGETS) {
	for (const variant of VARIANTS) {
		const label = `${variant}/${target}` as const;
		const size = file_size(new URL(`crates/tsv_wasm/pkg/${label}/tsv_wasm_bg.wasm`, root));
		if (size === null) {
			skip(`${label} — not built`);
			continue;
		}
		sizes[label] = size;
		const { min, max } = BOUNDS[variant];
		if (size < min) {
			fail(
				`${label}: ${format_size(size)} (${size} B) < min ${format_size(min)} — suspiciously small`,
			);
		} else if (size > max) {
			fail(
				`${label}: ${format_size(size)} (${size} B) > max ${format_size(max)} — size regression`,
			);
		} else {
			pass(`${label}: ${format_size(size)} (${size} B)`);
		}
	}
}

// Relative invariants per target: `all` is the superset build, so each
// subset must sit a stable margin below it.
for (const target of TARGETS) {
	const all_bytes = sizes[`all/${target}`];
	if (all_bytes === undefined) continue;
	for (const variant of ['format', 'parse'] as const) {
		const subset_bytes = sizes[`${variant}/${target}`];
		if (subset_bytes === undefined) continue;
		const { min, max } = DELTAS[variant];
		const delta = all_bytes - subset_bytes;
		if (delta < min) {
			fail(
				`all - ${variant} (${target}) = ${format_size(delta)} — expected ≥${
					format_size(min)
				} (feature gate broken?)`,
			);
		} else if (delta > max) {
			fail(
				`all - ${variant} (${target}) = ${format_size(delta)} — expected ≤${
					format_size(max)
				} (unexpected bloat)`,
			);
		} else {
			pass(`all - ${variant} (${target}) = ${format_size(delta)}`);
		}
	}
}

// --- Deno runtime smoke ---

interface SmokeTarget {
	label: string;
	entry: string;
	has_format: boolean;
	has_parse: boolean;
	/** The published npm package (patched by `patch_npm_package.ts`) vs the raw
	 * wasm-pack deno bundle. The pure-JS `locations.js` helper is patched into the
	 * npm packages only, so it's checked on npm entries alone. */
	is_npm: boolean;
}

const smoke_entries = (variant: 'format' | 'parse' | 'all'): SmokeTarget[] => [
	// npm package via its published Node entry (auto-init; Deno supports node:fs)
	{
		label: `${variant}/npm index.js`,
		entry: `crates/tsv_wasm/pkg/${variant}/npm/index.js`,
		has_format: variant !== 'parse',
		has_parse: variant !== 'format',
		is_npm: true,
	},
	// deno-target bundle (auto-init at import)
	{
		label: `${variant}/deno bundle`,
		entry: `crates/tsv_wasm/pkg/${variant}/deno/tsv_wasm.js`,
		has_format: variant !== 'parse',
		has_parse: variant !== 'format',
		is_npm: false,
	},
];

const smoke_targets: SmokeTarget[] = VARIANTS.flatMap(smoke_entries);

console.log('\n=== Deno runtime smoke ===');

for (const { label, entry, has_format, has_parse, is_npm } of smoke_targets) {
	const entry_url = new URL(entry, root);
	if (file_size(entry_url) === null) {
		skip(`${label} — not built`);
		continue;
	}
	let mod: Record<string, (source: string) => unknown>;
	try {
		mod = await import(entry_url.href);
	} catch (error) {
		fail(`${label} — import threw: ${error}`);
		continue;
	}
	if (has_format) {
		check(
			label,
			'format_typescript',
			() => mod.format_typescript('const   x=1') === 'const x = 1;\n',
		);
		check(label, 'format_css', () => mod.format_css('a{color:red}') === 'a {\n\tcolor: red;\n}\n');
		check(
			label,
			'format_svelte',
			() => mod.format_svelte('<div   >x</div   >') === '<div>x</div>\n',
		);
		check(label, 'IgnoreStack', () => {
			const ctor = (mod as Record<string, unknown>).IgnoreStack as
				| (new () => {
						push_gitignore(anchor: string, content: string): void;
						push_tsv(anchor: string, content: string): void;
						is_ignored(path: string, is_dir: boolean): boolean;
						classify_dir(name: string, child_rel: string, heuristic_active: boolean): string;
						should_format_file(name: string, child_rel: string): boolean;
				  })
				| undefined;
			if (!ctor) return false;
			const stack = new ctor();
			stack.push_gitignore('', 'build/\nignored.ts\n');
			stack.push_tsv('', '!build/keep.ts\n'); // tsv layer can't re-include under an excluded dir
			return (
				stack.is_ignored('build/x.ts', false) &&
				stack.is_ignored('build/keep.ts', false) &&
				!stack.is_ignored('src/x.ts', false) &&
				// the tsv_discover verdict methods (the format-only package's primary
				// discovery exports)
				stack.classify_dir('node_modules', 'node_modules', true) === 'prune' && // safety net
				stack.classify_dir('build', 'build', false) === 'prune' && // gitignored dir
				stack.classify_dir('src', 'src', false) === 'descend' &&
				stack.should_format_file('app.ts', 'src/app.ts') === true &&
				stack.should_format_file('ignored.ts', 'ignored.ts') === false && // leaf-matched ignore (should_format_file is leaf-only; build/ is kept out via classify_dir's dir prune above)
				stack.should_format_file('notes.md', 'notes.md') === false // wrong extension
			);
		});
	} else {
		check(label, 'format_* absent (parse-only build)', () => mod.format_typescript === undefined);
		check(
			label,
			'IgnoreStack absent (parse-only build)',
			() => (mod as Record<string, unknown>).IgnoreStack === undefined,
		);
	}
	if (has_parse) {
		check(
			label,
			'parse_typescript',
			() => (mod.parse_typescript('const x = 1;') as { type: string }).type === 'Program',
		);
		check(
			label,
			'parse_svelte',
			() => (mod.parse_svelte('<div>x</div>') as { type: string }).type === 'Root',
		);
		check(
			label,
			'parse_css',
			() => (mod.parse_css('a { color: red }') as { type: string }).type === 'StyleSheetFile',
		);
	}
	// The pure-JS `no-locations` reconstruction helper is patched into the npm
	// packages only (the raw wasm-pack deno bundle doesn't carry it), so scope this
	// to npm entries. Parse-side analog of IgnoreStack — Deno-runtime smoke that
	// test_npm.ts (Node) can't give: reconstruct must mutate in place and add loc.
	if (is_npm) {
		if (has_parse) {
			check(label, 'reconstruct_locations', () => {
				const reconstruct = (mod as Record<string, unknown>).reconstruct_locations as
					| ((ast: unknown, source: string) => { loc?: unknown })
					| undefined;
				if (typeof reconstruct !== 'function') return false;
				const ast = mod.parse_typescript_no_locations('const x = 1;') as { loc?: unknown };
				const out = reconstruct(ast, 'const x = 1;');
				return out === ast && !!out.loc; // same reference (in-place) + loc added
			});
		} else {
			check(
				label,
				'reconstruct_locations absent (format-only build)',
				() => (mod as Record<string, unknown>).reconstruct_locations === undefined,
			);
		}
	}
}

function check(target: string, name: string, assertion: () => boolean): void {
	try {
		if (assertion()) {
			pass(`${target} — ${name}`);
		} else {
			fail(`${target} — ${name} returned wrong output`);
		}
	} catch (error) {
		fail(`${target} — ${name} threw: ${error}`);
	}
}

// --- Summary ---

console.log(
	`\n=== Artifact validation: ${passed} passed, ${failed} failed, ${skipped} skipped ===`,
);
if (failed > 0) Deno.exit(1);
if (passed === 0) {
	console.error(
		'FAIL: no artifacts found to validate — run `deno task build:npm:format` etc. first',
	);
	Deno.exit(1);
}

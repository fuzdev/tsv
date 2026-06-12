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

// Measured 2026-06-11 (shape v2 — parse slimmed to parse-only, all = both
// features): format 2,196,623 B (npm); parse 1,699,885 B; all 2,889,210 B.
const BOUNDS = {
	format: { min: 2_020_000, max: 2_370_000 },
	parse: { min: 1_560_000, max: 1_840_000 },
	all: { min: 2_660_000, max: 3_120_000 },
};

// all = format + parse. `all − format` is the parse feature (parser convert +
// serde path; measured 692,587 B); `all − parse` is the format feature
// (printers + doc builder, dropped from the parse-only build at link time;
// measured 1,189,325 B). A delta near zero means a feature gate broke.
const DELTAS = {
	format: { min: 550_000, max: 850_000 }, // all − format
	parse: { min: 1_050_000, max: 1_300_000 }, // all − parse
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
}

const smoke_entries = (variant: 'format' | 'parse' | 'all'): SmokeTarget[] => [
	// npm package via its published Node entry (auto-init; Deno supports node:fs)
	{
		label: `${variant}/npm index.js`,
		entry: `crates/tsv_wasm/pkg/${variant}/npm/index.js`,
		has_format: variant !== 'parse',
		has_parse: variant !== 'format',
	},
	// deno-target bundle (auto-init at import)
	{
		label: `${variant}/deno bundle`,
		entry: `crates/tsv_wasm/pkg/${variant}/deno/tsv_wasm.js`,
		has_format: variant !== 'parse',
		has_parse: variant !== 'format',
	},
];

const smoke_targets: SmokeTarget[] = VARIANTS.flatMap(smoke_entries);

console.log('\n=== Deno runtime smoke ===');

for (const { label, entry, has_format, has_parse } of smoke_targets) {
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
	} else {
		check(label, 'format_* absent (parse-only build)', () => mod.format_typescript === undefined);
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

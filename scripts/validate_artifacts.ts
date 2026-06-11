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
 * - the `pkg/<variant>/deno/` bundles (the format one feeds nothing else
 *   that executes it — benches run only the parse build)
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

const VARIANTS = ['format', 'parse'] as const;
const TARGETS = ['npm', 'deno'] as const;

// Measured 2026-06-11 at v0.1.0: format 2,223,914 B (npm) / 2,223,935 B
// (deno); parse 2,917,712 B (npm) / 2,917,796 B (deno).
const BOUNDS = {
	format: { min: 2_050_000, max: 2_400_000 },
	parse: { min: 2_700_000, max: 3_150_000 },
};

// parse = format + the `ast` feature (parser + convert + serde path).
// Measured delta 693,798 B. A delta near zero means the feature gate broke.
const DELTA_MIN = 550_000;
const DELTA_MAX = 900_000;

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

// Relative invariant per target: parse carries the parser, so it must sit a
// stable margin above format.
for (const target of TARGETS) {
	const format_bytes = sizes[`format/${target}`];
	const parse_bytes = sizes[`parse/${target}`];
	if (format_bytes === undefined || parse_bytes === undefined) continue;
	const delta = parse_bytes - format_bytes;
	if (delta < DELTA_MIN) {
		fail(
			`parse - format (${target}) = ${format_size(delta)} — expected ≥${
				format_size(DELTA_MIN)
			} (ast feature gate broken?)`,
		);
	} else if (delta > DELTA_MAX) {
		fail(
			`parse - format (${target}) = ${format_size(delta)} — expected ≤${
				format_size(DELTA_MAX)
			} (unexpected bloat)`,
		);
	} else {
		pass(`parse - format (${target}) = ${format_size(delta)}`);
	}
}

// --- Deno runtime smoke ---

interface SmokeTarget {
	label: string;
	entry: string;
	has_parse: boolean;
}

const smoke_targets: SmokeTarget[] = [
	// npm packages via their published Node entry (auto-init; Deno supports node:fs)
	{
		label: 'format/npm index.js',
		entry: 'crates/tsv_wasm/pkg/format/npm/index.js',
		has_parse: false,
	},
	{ label: 'parse/npm index.js', entry: 'crates/tsv_wasm/pkg/parse/npm/index.js', has_parse: true },
	// deno-target bundles (auto-init at import)
	{
		label: 'format/deno bundle',
		entry: 'crates/tsv_wasm/pkg/format/deno/tsv_wasm.js',
		has_parse: false,
	},
	{
		label: 'parse/deno bundle',
		entry: 'crates/tsv_wasm/pkg/parse/deno/tsv_wasm.js',
		has_parse: true,
	},
];

console.log('\n=== Deno runtime smoke ===');

for (const { label, entry, has_parse } of smoke_targets) {
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
	check(
		label,
		'format_typescript',
		() => mod.format_typescript('const   x=1') === 'const x = 1;\n',
	);
	check(label, 'format_css', () => mod.format_css('a{color:red}') === 'a {\n\tcolor: red;\n}\n');
	check(label, 'format_svelte', () => mod.format_svelte('<div   >x</div   >') === '<div>x</div>\n');
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

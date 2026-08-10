/**
 * Size bounds for a staged N-API platform artifact
 * (`crates/tsv_napi/pkg/<triple>/tsv_napi.node`) — the native sibling of
 * `scripts/validate_artifacts.ts`'s wasm bounds, run per matrix target by the
 * release workflow (and locally after `deno task build:napi:packages`).
 *
 * Deliberately tight (~±8%) around a measured value, same philosophy as the
 * wasm bounds: a legitimate size change fails the release until the constant
 * moves, keeping binary-size drift visible and intentional. Triples marked
 * PROVISIONAL haven't shipped through the matrix yet — their wide bounds are
 * placeholders to be tightened to ±8% around the first release run's printed
 * sizes.
 *
 * Usage: deno run --allow-read scripts/validate_napi_artifact.ts [--triple <t>]
 * (default: the single staged platform dir under crates/tsv_napi/pkg/)
 */

import { parseArgs } from 'node:util';

const { values: args } = parseArgs({
	options: {
		triple: { type: 'string' }
	}
});

/** [min, max] bytes per triple. Measured = ±8% around a real artifact. */
const BOUNDS: Record<string, [number, number]> = {
	// band anchored at a measured 3,636,240 on linux (napi profile: release +
	// unwind), ±8% — the anchor, not a running figure
	'linux-x64-gnu': [3_345_000, 3_927_000],
	// PROVISIONAL — tighten to ±8% after the first release-matrix run
	'linux-arm64-gnu': [2_500_000, 5_500_000],
	'linux-x64-musl': [2_500_000, 5_500_000],
	'darwin-arm64': [2_000_000, 5_500_000],
	'win32-x64': [2_000_000, 5_500_000]
};

const pkg_root = 'crates/tsv_napi/pkg';
let triple = args.triple;
if (!triple) {
	const platform_dirs = [...Deno.readDirSync(pkg_root)]
		.filter((e) => e.isDirectory && e.name !== 'napi')
		.map((e) => e.name);
	if (platform_dirs.length !== 1) {
		console.error(
			`FAIL: pass --triple, or stage exactly one platform package (found: ${platform_dirs.join(', ') || 'none'})`
		);
		Deno.exit(1);
	}
	triple = platform_dirs[0];
}

const bounds = BOUNDS[triple!];
if (!bounds) {
	console.error(
		`FAIL: no size bounds for triple '${triple}' — add a (PROVISIONAL) entry to BOUNDS in scripts/validate_napi_artifact.ts`
	);
	Deno.exit(1);
}

const path = `${pkg_root}/${triple}/tsv_napi.node`;
let size: number;
try {
	size = Deno.statSync(path).size;
} catch {
	console.error(`FAIL: ${path} not staged — run 'deno task build:napi:packages' first`);
	Deno.exit(1);
}

const [min, max] = bounds;
const mb = (n: number) => `${(n / 1024 / 1024).toFixed(2)} MB`;
if (size < min || size > max) {
	console.error(
		`FAIL: ${triple} tsv_napi.node is ${size} bytes (${mb(size)}) — outside [${min}, ${max}]. ` +
			`A deliberate size change must move the bound in scripts/validate_napi_artifact.ts.`
	);
	Deno.exit(1);
}
console.log(`OK: ${triple} tsv_napi.node ${size} bytes (${mb(size)}) within [${min}, ${max}]`);

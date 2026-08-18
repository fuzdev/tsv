/**
 * Size bounds for the staged N-API platform artifacts
 * (`crates/tsv_napi/pkg/<triple>/` — the `tsv_napi.node` addon and the native
 * `tsv` CLI binary beside it) — the native sibling of
 * `scripts/validate_artifacts.ts`'s wasm bounds, run per matrix target by the
 * release workflow (and locally after `deno task build:napi:packages`).
 *
 * Deliberately tight (~±8%) around a measured value, same philosophy as the
 * wasm bounds: a legitimate size change fails the release until the constant
 * moves, keeping binary-size drift visible and intentional. All ten bands are
 * anchored on real matrix-built artifacts — re-anchor from a run's printed
 * sizes (this script logs them on success too) whenever a deliberate change
 * moves one, never widen a band to absorb drift.
 *
 * Usage: deno run --allow-read scripts/validate_napi_artifact.ts [--triple <t>]
 * (default: the single staged platform dir under crates/tsv_napi/pkg/)
 */

import { parseArgs } from 'node:util';

import { format_size } from './size.ts';

const { values: args } = parseArgs({
	options: {
		triple: { type: 'string' }
	}
});

/** [min, max] bytes per triple for `tsv_napi.node` (napi profile: release +
 * unwind). Every band is ±8% around a real artifact BUILT BY THE MATRIX — the
 * anchors below are the measured sizes, not running figures.
 *
 * Anchored on the first full matrix run, where each row is built in the
 * environment that ships it (the gnu rows in almalinux:8, musl in rust:alpine,
 * mac/win natively) — the only measurement the gate ever sees. A host build of
 * the same commit came within 1,840 B of the almalinux linux-x64-gnu figure, so
 * the container is not the size variable it might have looked like; the size
 * variable is the TARGET (win32/darwin sit ~10% under the linux rows). */
const BOUNDS: Record<string, [number, number]> = {
	'linux-x64-gnu': [3_458_000, 4_060_000], // 3,758,816
	'linux-arm64-gnu': [3_202_000, 3_760_000], // 3,480,696
	'linux-x64-musl': [3_460_000, 4_063_000], // 3,761,776
	'darwin-arm64': [3_061_000, 3_595_000], // 3,327,968
	'win32-x64': [3_437_000, 4_035_000] // 3,736,064
};

/** [min, max] bytes per triple for the native `tsv` CLI binary (`release`
 * profile: abort + LTO). Same anchoring discipline as `BOUNDS`, same run. */
const CLI_BOUNDS: Record<string, [number, number]> = {
	'linux-x64-gnu': [3_359_000, 3_944_000], // 3,651,832
	'linux-arm64-gnu': [3_082_000, 3_619_000], // 3,350,072
	'linux-x64-musl': [3_362_000, 3_947_000], // 3,654,360
	'darwin-arm64': [2_819_000, 3_311_000], // 3,065,088
	'win32-x64': [2_990_000, 3_511_000] // 3,250,688
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

const cli_binary_name = triple!.startsWith('win32-') ? 'tsv.exe' : 'tsv';

const validate = (filename: string, bounds: [number, number] | undefined): void => {
	if (!bounds) {
		console.error(
			`FAIL: no ${filename} size bounds for triple '${triple}' — add a (PROVISIONAL) entry in scripts/validate_napi_artifact.ts`
		);
		Deno.exit(1);
	}
	const path = `${pkg_root}/${triple}/${filename}`;
	let size: number;
	try {
		size = Deno.statSync(path).size;
	} catch {
		console.error(`FAIL: ${path} not staged — run 'deno task build:napi:packages' first`);
		Deno.exit(1);
	}
	const [min, max] = bounds;
	if (size < min || size > max) {
		console.error(
			`FAIL: ${triple} ${filename} is ${size} bytes (${format_size(size)}) — outside [${min}, ${max}]. ` +
				`A deliberate size change must move the bound in scripts/validate_napi_artifact.ts.`
		);
		Deno.exit(1);
	}
	console.log(
		`OK: ${triple} ${filename} ${size} bytes (${format_size(size)}) within [${min}, ${max}]`
	);
};

validate('tsv_napi.node', BOUNDS[triple!]);
validate(cli_binary_name, CLI_BOUNDS[triple!]);

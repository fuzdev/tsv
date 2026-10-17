/**
 * Size bounds for the staged N-API platform artifacts
 * (`crates/tsv_napi/pkg/<triple>/` — the `tsv_napi.node` addon and the native
 * `tsv` CLI binary beside it) — the native sibling of
 * `scripts/validate_artifacts.ts`'s wasm bounds, run per matrix target by the
 * release workflow, on the linux host build by check.yml's `artifacts` job, and
 * locally after `deno task build:napi:packages`.
 *
 * Deliberately tight (~±8%) around a measured value, same philosophy as the
 * wasm bounds: a legitimate size change fails the release until the constant
 * moves, keeping binary-size drift visible and intentional. Every band is
 * anchored on a real matrix-built artifact — re-anchor from a run's printed
 * sizes (this script logs them on success too) whenever a deliberate change
 * moves one, never widen a band to absorb drift. The one exception is a NEW
 * row's first run: it ships a wide PLACEHOLDER so the dispatch dry run passes
 * and prints the size, and the band is cut to ±8% around that figure before
 * the tag.
 *
 * Usage: deno run --allow-read scripts/validate_napi_artifact.ts [--triple <t>]
 * (default: the single staged platform dir under crates/tsv_napi/pkg/)
 */

import { parseArgs } from 'node:util';

import { cli_binary_name } from './napi_host.ts';
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
 * Anchored on a full matrix run, where each row is built in the environment
 * that ships it (the gnu rows in almalinux:8, musl in rust:alpine, win and
 * darwin-arm64 natively, darwin-x64 cross-compiled on the arm64 runner) — the
 * only measurement the gate ever sees. A host build of the same commit lands
 * within a few KB of the almalinux linux-x64-gnu figure, so the container is not
 * the size variable it might have looked like; the size variable is the TARGET
 * (win32/darwin-arm64 sit ~10% under the linux rows, darwin-x64 ~4%). */
const BOUNDS: Record<string, [number, number]> = {
	'linux-x64-gnu': [3_730_000, 4_380_000], // 4,055,224
	'linux-arm64-gnu': [3_443_000, 4_043_000], // 3,742,880
	'linux-x64-musl': [3_735_000, 4_386_000], // 4,060,784
	'darwin-arm64': [3_305_000, 3_881_000], // 3,593,376
	'darwin-x64': [3_453_000, 4_055_000], // 3,753,896
	'win32-x64': [3_716_000, 4_363_000] // 4,039,680
};

/** [min, max] bytes per triple for the native `tsv` CLI binary (`release`
 * profile: abort + LTO). Same anchoring discipline as `BOUNDS`, same runs. */
const CLI_BOUNDS: Record<string, [number, number]> = {
	'linux-x64-gnu': [3_609_000, 4_238_000], // 3,923,256
	'linux-arm64-gnu': [3_323_000, 3_902_000], // 3,612,816
	'linux-x64-musl': [3_610_000, 4_239_000], // 3,924,680
	'darwin-arm64': [3_003_000, 3_526_000], // 3,264,528
	'darwin-x64': [3_285_000, 3_857_000], // 3,571,048
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

const cli_binary = cli_binary_name(triple!);

/** Check one artifact against its band, logging the size either way; returns false on a
 * failure so both artifacts report before the exit (a failed addon must not hide the CLI's
 * size, which the next re-anchor needs). */
const validate = (filename: string, bounds: [number, number] | undefined): boolean => {
	if (!bounds) {
		console.error(
			`FAIL: no ${filename} size bounds for triple '${triple}' — add a band in scripts/validate_napi_artifact.ts, ±8% around a matrix-built artifact (a wide placeholder until the first run prints one)`
		);
		return false;
	}
	const path = `${pkg_root}/${triple}/${filename}`;
	let size: number;
	try {
		size = Deno.statSync(path).size;
	} catch {
		console.error(`FAIL: ${path} not staged — run 'deno task build:napi:packages' first`);
		return false;
	}
	const [min, max] = bounds;
	if (size < min || size > max) {
		console.error(
			`FAIL: ${triple} ${filename} is ${size} bytes (${format_size(size)}) — outside [${min}, ${max}]. ` +
				`A deliberate size change must move the bound in scripts/validate_napi_artifact.ts.`
		);
		return false;
	}
	console.log(
		`OK: ${triple} ${filename} ${size} bytes (${format_size(size)}) within [${min}, ${max}]`
	);
	return true;
};

const addon_ok = validate('tsv_napi.node', BOUNDS[triple!]);
const cli_ok = validate(cli_binary, CLI_BOUNDS[triple!]);
if (!addon_ok || !cli_ok) Deno.exit(1);

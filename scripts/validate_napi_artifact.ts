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
 * Two sets of bands. The plain ones grade a plain `cargo build` of both binaries —
 * what `deno task build:napi:packages` stages and check.yml's `artifacts` job builds
 * on every PR. `--pgo` grades the profile-guided pair that SHIPS
 * (`scripts/pgo/pgo.ts`, the release matrix): larger by the inlining a profile
 * drives, and bounded tightly for the same reason as everything here — the PGO-use
 * build turns LLVM's cost-benefit inliner off, and a toolchain or flag change that
 * quietly turned it back on would grow the binaries by far more than any band.
 *
 * Usage: deno run --allow-read scripts/validate_napi_artifact.ts [--triple <t>] [--pgo]
 * (default: the single staged platform dir under crates/tsv_napi/pkg/)
 */

import { parseArgs } from 'node:util';

import { cli_binary_name } from './napi_host.ts';
import { format_size } from './size.ts';

const { values: args } = parseArgs({
	options: {
		triple: { type: 'string' },
		pgo: { type: 'boolean', default: false }
	}
});

/** [min, max] bytes per triple for a PLAIN `tsv_napi.node` (napi profile: release +
 * unwind), ±8% around a measured artifact — the anchor is the measured size, not a running
 * figure. One row only: the plain addon is built by check.yml's `artifacts` job alone, on
 * linux-x64-gnu (an ubuntu runner, no container), and anchored on a host
 * `deno task build:napi:packages` build, which builds it the same way. The release matrix
 * builds no plain addon — every row ships and grades the PGO one (`PGO_BOUNDS`) — so no
 * other row has a band to keep honest, and a plain check of one is refused
 * (`PLAIN_ADDON_ROWS`). */
const BOUNDS: Record<string, [number, number]> = {
	'linux-x64-gnu': [4_067_000, 4_774_000] // 4,420,608
};

/** The rows something builds a plain addon for, named by the refusal. */
const PLAIN_ADDON_ROWS = Object.keys(BOUNDS);

/** [min, max] bytes per triple for the plain native `tsv` CLI binary (`release` profile:
 * abort + LTO), ±8% around a measured artifact. Every matrix row builds it — `pgo.ts`'s
 * byte-identity reference — and check.yml's `artifacts` job builds linux-x64-gnu's, which
 * is anchored on the same host build as `BOUNDS`. The other rows are anchored on an
 * earlier full matrix run, each built in the environment that ships it (the gnu rows in
 * almalinux:8, musl in rust:alpine, win and darwin-arm64 natively, darwin-x64
 * cross-compiled on the arm64 runner), and are re-anchored from the release workflow's
 * dispatch dry run. A host build lands within a few KB of the almalinux linux-x64-gnu
 * figure, so the container is not the size variable; the TARGET is. */
const CLI_BOUNDS: Record<string, [number, number]> = {
	'linux-x64-gnu': [3_929_000, 4_613_000], // 4,271,000
	'linux-arm64-gnu': [3_323_000, 3_902_000], // 3,612,816
	'linux-x64-musl': [3_610_000, 4_239_000], // 3,924,680
	'darwin-arm64': [3_003_000, 3_526_000], // 3,264,528
	'darwin-x64': [3_285_000, 3_857_000], // 3,571,048
	'win32-x64': [3_205_000, 3_763_000] // 3,484,160
};

/** [min, max] bytes per triple for the PGO-use `tsv_napi.node` (`--pgo`), anchored on
 * PGO-use builds.
 *
 * MEASURED rows — every linux and win32 row — are ±8% around the row's own PGO-use size:
 * linux-x64-gnu's a host PGO build of the source its plain anchor was measured on (the
 * matrix build lands within a few KB of it), the others a release-matrix build in the
 * environment that ships the row. The darwin rows are DERIVED until a matrix run prints
 * their sizes: linux-x64-gnu's measured PGO size scaled by the row's TARGET ratio — its
 * plain size over linux-x64-gnu's in the one matrix run the plain CLI anchors come from
 * (that run's plain addon sizes are noted beside the derived bands; its linux-x64-gnu
 * addon was 4,055,224 B, its CLI 3,923,256 B), a ratio of one vintage — ±10%, the wider
 * band for the ratio's uncertainty.
 * The band has two failures to tell apart, each measured on linux-x64-gnu against a plain
 * build of the same source: a profile that did not apply reads as a plain-sized binary
 * (PGO-use is about 1.2× plain — addon +21%, CLI +19%), and LLVM's cost-benefit inliner
 * turned back on as a much larger one (default PGO is +43% for the addon, +48% for the
 * CLI). A measured band spans [0.92, 1.08]× its row's PGO size, about [1.10, 1.30]× plain
 * where that ratio holds, so each failure falls outside with margin; the other measured
 * rows lean on linux-x64-gnu's PGO-over-plain ratio holding on their targets, which no
 * same-source plain build there has measured. A derived band leans on its row's target
 * ratio as well: while it holds, both failures fall outside a derived band too (by about
 * 7% at its nearer edge, the ±10% leaving less room than the measured ±8%); a ratio that
 * drifts can move either one inside, until the row is re-anchored to ±8% around its own
 * measured size. */
const PGO_BOUNDS: Record<string, [number, number]> = {
	'linux-x64-gnu': [4_906_000, 5_759_000], // 5,332,592
	'linux-arm64-gnu': [4_347_000, 5_103_000], // 4,725,112
	'linux-x64-musl': [4_908_000, 5_761_000], // 5,334,720
	'darwin-arm64': [4_253_000, 5_198_000], // derived 4,725,265 (plain 3,593,376)
	'darwin-x64': [4_443_000, 5_430_000], // derived 4,936,348 (plain 3,753,896)
	'win32-x64': [4_953_000, 5_814_000] // 5,383,168
};

/** [min, max] bytes per triple for the PGO-use `tsv` CLI binary (`--pgo`), anchored and
 * derived as `PGO_BOUNDS` is. */
const PGO_CLI_BOUNDS: Record<string, [number, number]> = {
	'linux-x64-gnu': [4_691_000, 5_507_000], // 5,099,096
	'linux-arm64-gnu': [4_226_000, 4_961_000], // 4,593,384
	'linux-x64-musl': [4_700_000, 5_517_000], // 5,108,424
	'darwin-arm64': [3_819_000, 4_667_000], // derived 4,242,940
	'darwin-x64': [4_177_000, 5_105_000], // derived 4,641,328
	'win32-x64': [4_253_000, 4_992_000] // 4,622,336
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
	if (!bounds && !args.pgo && filename === 'tsv_napi.node') {
		console.error(
			`FAIL: no plain tsv_napi.node band for '${triple}' — only ${PLAIN_ADDON_ROWS.join(', ')} ` +
				'builds a plain addon (check.yml); the release matrix ships and grades the PGO one, so grade ' +
				`it with --pgo. A plain addon of this row is graded by nothing, and a check of one is refused, not passed`
		);
		return false;
	}
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

const addon_ok = validate('tsv_napi.node', (args.pgo ? PGO_BOUNDS : BOUNDS)[triple!]);
const cli_ok = validate(cli_binary, (args.pgo ? PGO_CLI_BOUNDS : CLI_BOUNDS)[triple!]);
if (!addon_ok || !cli_ok) Deno.exit(1);

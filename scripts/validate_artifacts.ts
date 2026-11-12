/**
 * Validates the built WASM artifacts: binary sizes against expected ranges, and the
 * third-party `NOTICE` each npm package must ship (`THIRD_PARTY_NOTICE`).
 *
 * Size bounds are deliberately tight (~±8%) so a legitimate size change
 * fails here and gets acknowledged by updating the constants below — a
 * 100 KB regression (or win) should be visible, not absorbed by slack.
 *
 * It grades files, never runs one: what the packages do when loaded is the package suites'
 * (`scripts/test_npm.ts` under Node, `scripts/test_bun.ts` under Bun). The `deno` bundles it
 * sizes are the bench's Deno row's, sized beside the npm packages they share a `.wasm` with.
 *
 * Only validates artifacts that exist (skips unbuilt targets), but fails
 * if nothing was found at all — and refuses to grade a STALE one, since a
 * weeks-old `pkg/` sizes exactly as cleanly as a fresh one.
 *
 * Every bundle path comes from `benches/js/lib/tsv_artifacts.ts` — the same
 * builders the bench's size table, the run-side freshness guard, the build-side
 * skip and the three loaders resolve from. A release gate that spelled the layout
 * itself would keep passing over the old location after a move, which is the one
 * failure a size gate must not have.
 *
 * Usage: node scripts/roots/validate_artifacts.ts
 *
 * Exits 1 when a check fails, when nothing was found to validate, or when an artifact is
 * stale (unless `BENCH_STALE_OK=1`) or a staged file a check names is missing.
 *
 * Reach: the checkout read-only, and `BENCH_STALE_OK` (`ValidateArtifactsDeps`, built by
 * `scripts/roots/validate_artifacts.ts`).
 *
 * @module
 */

import { relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import type { FsReadDir } from '@fuzdev/gro/fs.ts';

import {
	CORE_CRATES,
	WASM_CRATES,
	wasm_bundle_dir,
	wasm_bundle_path,
	type WasmVariant
} from '../benches/js/lib/tsv_artifacts.ts';
import {
	type StagedCheck,
	staged_freshness_report,
	wasm_package_checks
} from './check_staged_freshness.ts';
import { handle_source_tree } from './handle_source_tree.ts';
import { THIRD_PARTY_NOTICE } from './npm_metadata.ts';
import { format_size } from './size.ts';

/** What `run_validate_artifacts` reaches. */
export interface ValidateArtifactsDeps {
	/** The checkout: every artifact and source is read beneath it. */
	checkout: FsReadDir;
	/** `BENCH_STALE_OK=1`: a stale artifact warns instead of failing the run. */
	stale_ok: boolean;
	print: (line: string) => void;
	print_error: (line: string) => void;
}

// --- WASM binary size checks ---

// The bundle LAYOUT is `lib/tsv_artifacts.ts`'s (`wasm_bundle_dir` /
// `wasm_bundle_path`); what varies here is the two axes this script walks. `npm` is
// a staging directory rather than a wasm-pack target, but it holds the same two
// files at the same place — `patch_npm_package.ts` copies the bundle in beside the
// entry it adds — so it rides the same builder rather than a second spelling.
const VARIANTS = ['format', 'parse', 'all'] as const satisfies readonly WasmVariant[];
const TARGETS = ['npm', 'deno'] as const;

/** The handle takes checkout-relative paths; the LAYOUT still comes from
 * `tsv_artifacts.ts`, so a move stays a one-place edit. */
const repo_rel = (absolute: string): string =>
	relative(fileURLToPath(new URL('..', import.meta.url)), absolute);

/** A variant's bundle for a target, checkout-relative. */
const bundle_path = (variant: WasmVariant, target: string): string =>
	repo_rel(wasm_bundle_path(variant, target));

/** A variant's npm staging manifest, checkout-relative. */
const npm_manifest = (variant: WasmVariant): string =>
	`${repo_rel(wasm_bundle_dir(variant, 'npm'))}/package.json`;

/**
 * The files any staging step leaves in a variant's npm staging, checkout-relative: those the
 * shared checks name (the bundle and the entries), the manifest, and the worker entry. One of
 * them present makes the staging built.
 */
const npm_staged_files = (variant: WasmVariant): Array<string> => [
	...wasm_package_checks(variant).map((check) => check.staged),
	npm_manifest(variant),
	`${repo_rel(wasm_bundle_dir(variant, 'npm'))}/worker.js`
];

const REBUILD_TASKS = {
	// the format variant is the unqualified one (`deno task build:wasm:deno`)
	deno: { format: 'build:wasm:deno', parse: 'build:wasm:parse:deno', all: 'build:wasm:all:deno' }
} as const;

/**
 * The freshness checks over what is built: a deno bundle is a wasm-pack output and nothing
 * more; an npm staging is graded by the one list every reader of a staged package shares.
 * Existence-gated like every check below, so an unbuilt variant is skipped, not failed.
 */
export function artifact_freshness_checks(built: {
	deno: ReadonlySet<WasmVariant>;
	npm: ReadonlySet<WasmVariant>;
}): Array<StagedCheck> {
	const checks: Array<StagedCheck> = [];
	for (const variant of VARIANTS) {
		if (built.deno.has(variant)) {
			checks.push({
				label: `${variant}/deno WASM bundle`,
				staged: bundle_path(variant, 'deno'),
				crates: [...CORE_CRATES, ...WASM_CRATES],
				// `package.json` because the wasm-pack invocation lives there: which
				// FEATURES a variant is built with is a task string, so editing one
				// changes the bundle from unchanged crate sources. `run_if_stale.ts`
				// — the build-side guard over this same artifact, off the same two
				// crate lists — names it for that reason, and the two sides are
				// supposed to agree on what feeds a bundle.
				files: ['package.json'],
				rebuild: `deno task ${REBUILD_TASKS.deno[variant]}`
			});
		}
		if (built.npm.has(variant)) checks.push(...wasm_package_checks(variant));
	}
	return checks;
}

// Centers (deno target; npm == deno, identical `.wasm`): format 2,720,874 B, parse
// 1,073,871 B and all 3,048,129 B, measured on one tree 2026-11-03; all carries the type
// strip and the import records (the `strip` and `imports` features, which the umbrella alone
// builds). Bounds are ±8% of each. The import records are about 10 KB of `all`, and no
// part of format or parse.
//
// Recenter (and re-measure the deltas below) whenever a measure has drifted to
// within a few percent of a band edge through accumulated work — otherwise the
// next release discovers it as a publish failure instead of as a visible size
// move. A real size step (a cut or a regression traceable to one change) is what
// the tight band exists to surface; recenter on it deliberately, with the cause
// named in the commit. The retired centers and what moved them are recorded
// elsewhere, not here.
export const BOUNDS = {
	format: { min: 2_503_000, max: 2_938_000 },
	parse: { min: 988_000, max: 1_160_000 },
	all: { min: 2_804_000, max: 3_292_000 }
};

// all = format + parse. `all − format` is what the parse feature adds (parser
// convert path); `all − parse` is what the format feature adds (printers + doc
// builder, dropped from the parse-only build at link time) — the gate-health
// signal. A delta near zero means a feature gate broke. Read a delta as the
// feature's weight MINUS whatever both variants share (`js-sys` is linked by the
// parse feature alone — the format exports read no options bag — so it counts in
// `all − format`). A change that lands in code both variants link (a parser cut) moves the
// bundles and leaves the deltas where they were; a delta that moves with the
// bundles means a feature boundary shifted, not that code got smaller. The `strip` and
// `imports` features ride `all` alone, so the type strip and the import records count in
// both deltas: `all − format` is the parse wire, the type strip and the import records
// together, the import records its smallest part.
//
// At the current center: `all − format` 327,255 B; `all − parse` 1,974,258 B.
export const DELTAS = {
	format: { min: 301_000, max: 353_000 }, // all − format
	parse: { min: 1_816_000, max: 2_132_000 } // all − parse
};

/**
 * Validates what is built (module doc).
 * @returns the exit code: `0` when every check passed, `1` otherwise
 */
export async function run_validate_artifacts(deps: ValidateArtifactsDeps): Promise<number> {
	const { checkout, print } = deps;
	let passed = 0;
	let failed = 0;
	let skipped = 0;

	const pass = (msg: string): void => {
		print(`  PASS: ${msg}`);
		passed++;
	};
	const fail = (msg: string): void => {
		print(`  FAIL: ${msg}`);
		failed++;
	};
	const skip = (msg: string): void => {
		print(`  SKIP: ${msg}`);
		skipped++;
	};
	const check = async (
		target: string,
		name: string,
		assertion: () => Promise<boolean>
	): Promise<void> => {
		try {
			if (await assertion()) {
				pass(`${target} — ${name}`);
			} else {
				fail(`${target} — ${name} returned wrong output`);
			}
		} catch (error) {
			fail(`${target} — ${name} threw: ${error}`);
		}
	};
	/** The size of the entry at `path`, or `null` when nothing is there. */
	const file_size = async (path: string): Promise<number | null> =>
		(await checkout.stat(path))?.size ?? null;

	// --- Staleness guard ---
	//
	// Everything below reads BUILT files, and nothing about a stale one looks wrong:
	// an old bundle has a size and the notice it was staged with. The release path is safe
	// by construction (`publish.ts` builds first, as does CI), but the bare
	// `validate:artifacts` is also how one asks "are the artifacts OK?" — and against a
	// weeks-old `pkg/` it answers about code that no longer exists. (`pkg/` is gitignored,
	// so nothing else makes its age visible.) This is the same guard the staged-package
	// tests use, over the same two lags, so the trap keeps one spelling.
	//
	// `BENCH_STALE_OK=1` downgrades staleness to a warning, the one knob every mtime guard
	// in the repo shares.
	//
	// A deno bundle counts as built once its `.wasm` is there, an npm staging once any file a
	// staging step writes is (`npm_staged_files`), so a staging holding some without the rest
	// fails rather than being skipped as unbuilt.
	const built = { deno: new Set<WasmVariant>(), npm: new Set<WasmVariant>() };
	for (const variant of VARIANTS) {
		if ((await file_size(bundle_path(variant, 'deno'))) !== null) built.deno.add(variant);
		for (const path of npm_staged_files(variant)) {
			if ((await file_size(path)) !== null) {
				built.npm.add(variant);
				break;
			}
		}
	}
	const freshness = await staged_freshness_report(artifact_freshness_checks(built), {
		tree: handle_source_tree(checkout),
		stale_ok: deps.stale_ok
	});
	if (freshness !== undefined) {
		deps.print_error(freshness.message);
		if (freshness.fatal) return 1;
	}

	print('=== WASM binary sizes ===');

	const sizes: Partial<Record<`${(typeof VARIANTS)[number]}/${(typeof TARGETS)[number]}`, number>> =
		{};

	for (const target of TARGETS) {
		for (const variant of VARIANTS) {
			const label = `${variant}/${target}` as const;
			const size = await file_size(bundle_path(variant, target));
			if (size === null) {
				skip(`${label} — not built`);
				continue;
			}
			sizes[label] = size;
			const { min, max } = BOUNDS[variant];
			if (size < min) {
				fail(
					`${label}: ${format_size(size)} (${size} B) < min ${format_size(min)} — suspiciously small`
				);
			} else if (size > max) {
				fail(
					`${label}: ${format_size(size)} (${size} B) > max ${format_size(max)} — size regression`
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
					`all - ${variant} (${target}) = ${format_size(delta)} — expected ≥${format_size(
						min
					)} (feature gate broken?)`
				);
			} else if (delta > max) {
				fail(
					`all - ${variant} (${target}) = ${format_size(delta)} — expected ≤${format_size(
						max
					)} (unexpected bloat)`
				);
			} else {
				pass(`all - ${variant} (${target}) = ${format_size(delta)}`);
			}
		}
	}

	// --- Third-party notice ---
	//
	// Every npm package compiles in the WHATWG entity table, whose license requires its notice
	// in the documentation of a binary distribution (`THIRD_PARTY_NOTICE`). A package without
	// it is not publishable, however well it sizes.

	print('\n=== Third-party notice ===');

	const notice_source = await checkout.read_text(THIRD_PARTY_NOTICE.source);
	for (const variant of VARIANTS) {
		const dir = repo_rel(wasm_bundle_dir(variant, 'npm'));
		const label = `${variant}/npm`;
		if (!built.npm.has(variant)) {
			skip(`${label} NOTICE — not built`);
			continue;
		}
		// a staging with its bundle and no manifest declares nothing
		await check(label, `package.json files declares ${THIRD_PARTY_NOTICE.published}`, async () => {
			const manifest = await checkout.read_text_if_exists(npm_manifest(variant));
			if (manifest === null) return false;
			const { files } = JSON.parse(manifest) as { files?: string[] };
			return (files ?? []).includes(THIRD_PARTY_NOTICE.published);
		});
		await check(
			label,
			`${THIRD_PARTY_NOTICE.published} is ${THIRD_PARTY_NOTICE.source} verbatim`,
			async () =>
				(await file_size(`${dir}/${THIRD_PARTY_NOTICE.published}`)) !== null &&
				(await checkout.read_text(`${dir}/${THIRD_PARTY_NOTICE.published}`)) === notice_source
		);
	}

	// --- Summary ---

	print(`\n=== Artifact validation: ${passed} passed, ${failed} failed, ${skipped} skipped ===`);
	if (failed > 0) return 1;
	if (passed === 0) {
		deps.print_error(
			'FAIL: no artifacts found to validate — run `deno task build:npm:format` etc. first'
		);
		return 1;
	}
	return 0;
}

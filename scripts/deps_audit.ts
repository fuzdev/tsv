/**
 * The published-graph dependency audit: no crate meant for publishing may reach a denied
 * package through a normal or build dependency, at any feature set or on any target.
 *
 * ## What it proves
 *
 * That a consumer of a published tsv crate never builds a denied package because of tsv.
 * The denied list is the workspace dependencies whose entry carries features that would
 * unify into a consumer's own copy and change its behavior: `serde_json`, whose
 * `preserve_order` + `float_roundtrip` + `arbitrary_precision` change how a consumer's
 * `serde_json` deserializes (an internally tagged enum holding an `f64` stops
 * deserializing, `1.0` and `1.00` compare unequal, key order changes). Those features
 * serve the fixture gate (`tsv_debug`), so the entry keeps them, and this audit keeps
 * `serde_json` to dev-dependencies and unpublished crates. A dev-dependency never reaches
 * a consumer; neither does an unpublished (`publish = false`) crate (`tsv_ffi`, `tsv_napi`,
 * `tsv_wasm`, `tsv_debug`, the root test package `tsv_tests`).
 *
 * ## Which crates, how
 *
 * The crates meant for publishing are the members with a `[package.metadata.docs.rs]`
 * table (`docsrs_crates.ts`, the definition `docs:audit:docsrs` shares). For each, one
 * `cargo tree -p <crate> -e normal,build --all-features --target all` lists every package
 * its normal and build edges can reach: features are additive, so `--all-features`
 * reaches every package any feature subset can, and `--target all` keeps target-specific
 * dependencies in. A denied package in that list fails the crate, and the inverted tree
 * (`-i`) names the path to it.
 *
 * ## Blind spots
 *
 * Only the listed packages are graded: a NEW workspace dependency with consumer-visible
 * features is not denied until it is added here. A publishable crate that lacks the docs.rs
 * table is not skipped: the shared reader refuses a member that is neither tabled nor
 * `publish = false`, so the run fails instead.
 * `cargo tree` resolves against `Cargo.lock`, which a consumer's resolution can differ
 * from, but a denied package is reached by a dependency edge tsv declares, which no
 * lockfile removes.
 *
 * ## Reach
 *
 * The checkout read-only (the manifests) and `cargo`, which runs `cargo tree` in it
 * (`DepsAuditDeps`, built by `scripts/roots/deps_audit.ts`).
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { docs_rs_crates } from './docsrs_crates.ts';

const log = (...args: unknown[]) => console.error(...args);

/** The packages no crate meant for publishing may reach through a normal or build edge. */
const DENIED = ['serde_json'];

/** What `run_deps_audit` reaches. */
export interface DepsAuditDeps {
	/** The checkout, read for its manifests and cargo's working directory. */
	checkout: FsReadDir;
	/** `cargo`, its output captured. */
	cargo: SpawnProgram;
}

/** `cargo tree` over `crate_name`'s normal and build edges, every feature and target. */
async function cargo_tree(
	deps: DepsAuditDeps,
	crate_name: string,
	extra: string[]
): Promise<{ status: number; stdout: string }> {
	const result = await deps.cargo.out(
		['tree', '-p', crate_name, '-e', 'normal,build', '--all-features', '--target', 'all', ...extra],
		{ cwd: deps.checkout }
	);
	const status = result.kind === 'exited' ? result.code : 1;
	if (status !== 0) log(result.stderr.trimEnd());
	return { status, stdout: result.stdout };
}

/**
 * The audit (module doc).
 * @returns the exit code: `1` when a crate reaches a denied package or isn't graded, else `0`
 */
export async function run_deps_audit(deps: DepsAuditDeps): Promise<number> {
	const crates = docs_rs_crates((path) => deps.checkout.read_text_sync(path));
	if (crates.length === 0) {
		log('Error: no workspace crate carries a [package.metadata.docs.rs] table — nothing graded.');
		return 1;
	}

	let failed = 0;
	for (const c of crates) {
		const tree = await cargo_tree(deps, c.name, ['--prefix', 'none', '--format', '{p}']);
		if (tree.status !== 0) {
			failed++;
			log(`✗ ${c.name}: cargo tree failed`);
			continue;
		}
		const reached = new Set(
			tree.stdout
				.split('\n')
				.filter((line) => line !== '')
				.map((line) => line.split(' ')[0])
		);
		// The listing is the crate's own graph, so it names the crate itself; one that names
		// nothing (an empty or failed listing) grades nothing.
		if (!reached.has(c.name)) {
			failed++;
			log(`✗ ${c.name}: cargo tree listed no graph for it`);
			continue;
		}
		const denied = DENIED.filter((name) => reached.has(name));
		if (denied.length > 0) {
			failed++;
			for (const name of denied) {
				log((await cargo_tree(deps, c.name, ['-i', name])).stdout.trimEnd());
			}
			log(`✗ ${c.name}: reaches ${denied.join(', ')} through a normal or build dependency`);
		} else {
			log(`✓ ${c.name}: ${reached.size - 1} package(s) reached, none denied`);
		}
	}
	if (failed > 0) {
		log(`deps:audit FAILED — ${failed} of ${crates.length} crate(s)`);
		return 1;
	}
	log(`deps:audit OK — ${crates.length} crate(s) reach none of: ${DENIED.join(', ')}`);
	return 0;
}

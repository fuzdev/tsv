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
 * a consumer; neither does an unpublished crate (`tsv_ffi`, `tsv_napi`, `tsv_wasm`,
 * `tsv_debug`, the root test package).
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
 * features is not denied until it is added here. A crate meant for publishing that lacks
 * the docs.rs table is not graded at all (the same blind spot as `docs:audit:docsrs`).
 * `cargo tree` resolves against `Cargo.lock`, which a consumer's resolution can differ
 * from, but a denied package is reached by a dependency edge tsv declares, which no
 * lockfile removes.
 */

import { spawnSync } from 'node:child_process';

import { docs_rs_crates } from './docsrs_crates.ts';

const log = (...args: unknown[]) => console.error(...args);

/** The packages no crate meant for publishing may reach through a normal or build edge. */
const DENIED = ['serde_json'];

/** `cargo tree` over `crate_name`'s normal and build edges, every feature and target. */
function cargo_tree(crate_name: string, extra: string[]): { status: number; stdout: string } {
	const result = spawnSync(
		'cargo',
		['tree', '-p', crate_name, '-e', 'normal,build', '--all-features', '--target', 'all', ...extra],
		{ encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }
	);
	if (result.status !== 0) log(result.stderr.trimEnd());
	return { status: result.status ?? 1, stdout: result.stdout ?? '' };
}

function run(): never {
	const crates = docs_rs_crates();
	if (crates.length === 0) {
		log('Error: no workspace crate carries a [package.metadata.docs.rs] table — nothing graded.');
		process.exit(1);
	}

	let failed = 0;
	for (const c of crates) {
		const tree = cargo_tree(c.name, ['--prefix', 'none', '--format', '{p}']);
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
				log(cargo_tree(c.name, ['-i', name]).stdout.trimEnd());
			}
			log(`✗ ${c.name}: reaches ${denied.join(', ')} through a normal or build dependency`);
		} else {
			log(`✓ ${c.name}: ${reached.size - 1} package(s) reached, none denied`);
		}
	}
	if (failed > 0) {
		log(`deps:audit FAILED — ${failed} of ${crates.length} crate(s)`);
		process.exit(1);
	}
	log(`deps:audit OK — ${crates.length} crate(s) reach none of: ${DENIED.join(', ')}`);
	process.exit(0);
}

run();

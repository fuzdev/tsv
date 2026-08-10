/**
 * The JS-CLI half of the shared discovery-parity scenario table — one runner
 * registered by both package test suites, so the two cli.js copies are held to
 * the same table by the same code. `tests/discovery_parity.rs` runs the SAME
 * `tests/discovery/scenarios.json` through the native `discover_files`, so the
 * three discovery walkers (native CLI, wasm cli.js, napi cli.js) can't drift:
 * a divergence fails one side or the other.
 *
 * Each scenario materializes its `tree` in a tempdir (string = file, null =
 * empty dir; a `.git` entry fakes a repo root with no real git binary), then
 * asserts `format --list <root>/<target>` reports `expected` (root-relative,
 * exact sorted order) — or, for a case carrying `error` instead, that the run
 * fails upfront (exit 2) with that substring on stderr and nothing on stdout.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

/** Materialize a scenario `tree`: string = file (parents created), null = empty dir. */
const materialize = (root: string, tree: Record<string, string | null>): void => {
	for (const [rel, value] of Object.entries(tree)) {
		const path = join(root, rel);
		if (value === null) {
			mkdirSync(path, { recursive: true });
		} else {
			mkdirSync(dirname(path), { recursive: true });
			writeFileSync(path, value);
		}
	}
};

/**
 * Separators normalized to `/`, the spelling `expected` is written in.
 * Discovery emits *native* separators (`PathBuf::push` parity), so on Windows
 * both the reported paths and the tempdir root prefix come back `\`-joined and
 * the strip below would never match — every case reading back absolute. The
 * native harness (`tests/discovery_parity.rs`) normalizes the same two sides
 * for the same reason; `\` is a legal posix filename byte, but no scenario
 * name uses one.
 */
const to_posix = (path: string): string => path.replaceAll('\\', '/');

/** Register the parity table as a `describe` block driving `cli_path`. */
export const register_discovery_parity_suite = (
	name: string,
	cli_path: string,
	options?: { skip?: boolean }
): void => {
	describe(name, { skip: options?.skip ?? false }, () => {
		const table = JSON.parse(
			readFileSync(new URL('../tests/discovery/scenarios.json', import.meta.url), 'utf-8')
		);

		for (const scenario of table.scenarios) {
			it(scenario.name, () => {
				const root = mkdtempSync(join(tmpdir(), 'tsv-parity-'));
				try {
					materialize(root, scenario.tree);
					const prefix = `${to_posix(root)}/`;
					for (const { target, expected, error } of scenario.cases) {
						const arg = target === '' ? root : join(root, target);
						const result = spawnSync(process.execPath, [cli_path, 'format', '--list', arg], {
							encoding: 'utf-8'
						});
						if (error !== undefined) {
							assert.equal(
								result.status,
								2,
								`${scenario.name} [target=${target}]: expected exit 2`
							);
							assert.ok(
								result.stderr.includes(error),
								`${scenario.name} [target=${target}]: expected stderr to contain ${JSON.stringify(error)}, got ${JSON.stringify(result.stderr)}`
							);
							assert.equal(result.stdout.trim(), '', `${scenario.name} [target=${target}]`);
							continue;
						}
						assert.equal(result.status, 0, `${scenario.name} [${target}]: ${result.stderr}`);
						const actual = result.stdout
							.split('\n')
							.filter((line) => line !== '')
							.map(to_posix)
							.map((line) => (line.startsWith(prefix) ? line.slice(prefix.length) : line));
						assert.deepEqual(actual, expected, `${scenario.name} [target=${target}]`);
					}
				} finally {
					rmSync(root, { recursive: true, force: true });
				}
			});
		}
	});
};

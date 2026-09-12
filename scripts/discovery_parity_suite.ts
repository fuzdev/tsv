/**
 * The JS-CLI half of the shared discovery-parity scenario table — one runner
 * registered by both package test suites, so the two cli.js copies are held to
 * the same table by the same code. `tests/discovery_parity.rs` runs the SAME
 * `tests/discovery/scenarios.json` through the native `discover_files`, so the
 * three discovery walkers (native CLI, wasm cli.js, napi cli.js) can't drift:
 * a divergence fails one side or the other.
 *
 * Each scenario materializes its `tree` in a tempdir (string = file, null =
 * empty dir, `{symlink: target}` = a symlink, skipped on Windows; a `.git` entry
 * fakes a repo root with no real git binary), then asserts
 * `format --list <root>/<target>` reports `expected` (root-relative,
 * exact sorted order) — or, for a case carrying `error` instead, that the run
 * fails upfront (exit 2) with that substring on stderr and nothing on stdout.
 *
 * A case may carry a `targets` LIST in place of its single `target`, which is the only
 * way the table reaches what a *set* of arguments decides: the one ignore scope file
 * arguments share as it moves from each one's directory to the next's (the native
 * `FileScope` / `enter_file_scope`), and the canonical-path dedup only overlapping roots
 * run. Those were pinned by a hand-mirrored test on each bin before the shape existed —
 * which is the drift this table exists to remove.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

/** One scenario tree entry: a file's contents, an empty directory, or a symlink. */
type TreeValue = string | null | { symlink: string };

/** Materialize a scenario `tree`: string = file (parents created), null = empty dir,
 * `{symlink: target}` = a symbolic link to `target`, resolved from the link's own
 * directory (a scenario holding one is skipped on Windows — `holds_symlinks`). */
const materialize = (root: string, tree: Record<string, TreeValue>): void => {
	for (const [rel, value] of Object.entries(tree)) {
		const path = join(root, rel);
		if (value === null) {
			mkdirSync(path, { recursive: true });
			continue;
		}
		mkdirSync(dirname(path), { recursive: true });
		if (typeof value === 'string') writeFileSync(path, value);
		else symlinkSync(value.symlink, path);
	}
};

/** Whether a scenario's tree holds a symlink, which Windows can't make unprivileged. */
const holds_symlinks = (tree: Record<string, TreeValue>): boolean =>
	Object.values(tree).some((value) => value !== null && typeof value === 'object');

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
			const skip = process.platform === 'win32' && holds_symlinks(scenario.tree);
			it(scenario.name, { skip }, () => {
				const root = mkdtempSync(join(tmpdir(), 'tsv-parity-'));
				try {
					materialize(root, scenario.tree);
					const prefix = `${to_posix(root)}/`;
					for (const { target, targets, expected, error, warns, no_warns } of scenario.cases) {
						const list: Array<string> = targets ?? [target];
						const args = list.map((t: string) => (t === '' ? root : join(root, t)));
						// what a failure names the case by: the one target, or the whole list
						const label = list.join(' ');
						const result = spawnSync(process.execPath, [cli_path, 'format', '--list', ...args], {
							encoding: 'utf-8'
						});
						if (error !== undefined) {
							assert.equal(result.status, 2, `${scenario.name} [target=${label}]: expected exit 2`);
							assert.ok(
								result.stderr.includes(error),
								`${scenario.name} [target=${label}]: expected stderr to contain ${JSON.stringify(error)}, got ${JSON.stringify(result.stderr)}`
							);
							assert.equal(result.stdout.trim(), '', `${scenario.name} [target=${label}]`);
							continue;
						}
						assert.equal(result.status, 0, `${scenario.name} [${label}]: ${result.stderr}`);
						const actual = result.stdout
							.split('\n')
							.filter((line) => line !== '')
							.map(to_posix)
							.map((line) => (line.startsWith(prefix) ? line.slice(prefix.length) : line));
						assert.deepEqual(actual, expected, `${scenario.name} [target=${label}]`);
						// `warns`: substrings each of which some `warning:` line must carry
						for (const needle of warns ?? []) {
							assert.ok(
								result.stderr.includes(needle),
								`${scenario.name} [target=${label}]: expected a warning containing ${JSON.stringify(needle)}, stderr: ${JSON.stringify(result.stderr)}`
							);
						}
						// `no_warns`: substrings no warning may carry
						for (const needle of no_warns ?? []) {
							assert.ok(
								!result.stderr.includes(needle),
								`${scenario.name} [target=${label}]: expected no warning containing ${JSON.stringify(needle)}, stderr: ${JSON.stringify(result.stderr)}`
							);
						}
					}
				} finally {
					rmSync(root, { recursive: true, force: true });
				}
			});
		}
	});
};

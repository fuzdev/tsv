/**
 * Harvest freshness stamps — skip a re-harvest when its INPUTS are unchanged.
 *
 * Each harvest writes a stamp JSON beside its cache recording every input its
 * output depends on: the SOURCE CHECKOUT COMMIT(S) (`git_head` — upstream
 * version files only bump at release, so the commit is the only precise input
 * statement), plus the pinned expected count(s) and oracle versions that shape
 * the output. On the next run, matching inputs + an existing cache → skip
 * (logged); anything else → full re-harvest, and the stamp is written only
 * AFTER the harvest and its pinned-count check succeed, so a failed or
 * wrong-sized harvest never stamps itself fresh.
 *
 * `--force` re-harvests regardless — needed when the harvest's own LOGIC
 * changes without moving any keyed input (e.g. a grading/extraction change in
 * the script or, for test262, in the Rust runner). Stamps live in
 * `benches/js/.cache` (gitignored); `deno task bench:clean` wipes them with
 * the caches.
 *
 * Deno-only (like the harvests themselves — `git` runs via `Deno.Command`,
 * needing `--allow-run=git`).
 */

import { readFile, stat, writeFile } from 'node:fs/promises';

export type StampInputs = Record<string, string | number | null>;

/** `HEAD` commit of a checkout, or null when it isn't a git repo / git fails. */
export function git_head(repo: string): string | null {
	try {
		const out = new Deno.Command('git', {
			args: ['-C', repo, 'rev-parse', 'HEAD'],
			stdout: 'piped',
			stderr: 'null',
		}).outputSync();
		if (!out.success) return null;
		return new TextDecoder().decode(out.stdout).trim();
	} catch {
		return null;
	}
}

/** First 9 chars of a full commit SHA — the short form used in harvest skip logs. */
export function short_commit(sha: string): string {
	return sha.slice(0, 9);
}

/**
 * Whether an up-to-date cache already exists: every path in `caches` is present
 * on disk AND the stamp at `stamp_path` records exactly `inputs`. The freshness
 * gate a harvest checks before doing work — the caller keeps the `--force` /
 * log / skip decision around it (skip is `Deno.exit(0)` or a `return`, and the
 * log wording differs per harvest).
 */
export async function harvest_up_to_date(
	stamp_path: string,
	inputs: StampInputs,
	caches: string[],
): Promise<boolean> {
	for (const path of caches) {
		try {
			await stat(path);
		} catch {
			return false; // absent cache → harvest
		}
	}
	return stamp_fresh(stamp_path, inputs);
}

/** Whether the stamp at `path` records exactly `inputs`. */
export async function stamp_fresh(path: string, inputs: StampInputs): Promise<boolean> {
	try {
		const recorded = JSON.parse(await readFile(path, 'utf8')) as StampInputs;
		const keys = Object.keys(inputs);
		return (
			keys.length === Object.keys(recorded).length &&
			keys.every((k) => recorded[k] === inputs[k])
		);
	} catch {
		return false;
	}
}

/** Record `inputs` at `path` — call only after the harvest + its pin check succeed. */
export async function write_stamp(path: string, inputs: StampInputs): Promise<void> {
	await writeFile(path, JSON.stringify(inputs, null, '\t') + '\n');
}

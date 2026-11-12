/**
 * Harvest freshness stamps — skip a re-harvest when its INPUTS are unchanged.
 *
 * Each harvest writes a stamp JSON beside its cache recording every input its
 * output depends on: the SOURCE CHECKOUT OBJECT(S) (`checkout_hash` — a
 * checkout's HEAD, or one subtree's id at HEAD; upstream version files only bump
 * at release, so the git object is the only precise input statement), the pinned
 * expected count(s) and oracle versions that shape the output, and — for a grade
 * that loads a corpus VIEW — that view's entry list (`corpus_view_paths`), which
 * no checkout records. On the next run, matching inputs + an existing cache → skip
 * (logged); anything else → full re-harvest, and the stamp is written only
 * AFTER the harvest and its pinned-count check succeed, so a failed or
 * wrong-sized harvest never stamps itself fresh.
 *
 * `--force` re-harvests regardless — needed when the harvest's own LOGIC
 * changes without moving any keyed input (e.g. a grading/extraction change in
 * the script or, for test262, in the Rust runner). The one piece of logic a
 * harvest does NOT own but reads through — the corpus loader's per-file filters
 * (`corpus_filter_fingerprint`) — is stamped by every grade that loads a corpus
 * view, so a filter change re-harvests without anyone remembering `--force`.
 * Stamps live in `benches/js/.cache` (gitignored); `deno task bench:clean` wipes
 * them with the caches. Which stamps exist, and where, is `harvest_stamp_table.ts`.
 *
 * Every read, write, and `git` call goes through what the caller hands in: `StampGit`, a
 * `git` whose `rev-parse` reads the local repo only (a gro `SpawnProgram` satisfies it, made
 * with `GIT_LOCAL_PASS_ENV`), and `StampReads` / `StampWrites`, the checkout's files by
 * project-root-relative path (a gro `FsReadDir` / `FsDir` on the checkout satisfies them).
 * The types are stated here rather than imported, so the module imports no package.
 *
 * @module
 */

import { createHash } from 'node:crypto';
import { dirname } from 'node:path';

import type { CheckoutInput } from './harvest_stamp_table.ts';

export type StampInputs = Record<string, string | number | null>;

/** `git`, its output captured as text — a gro `SpawnProgram` is one. */
export interface StampGit {
	out(
		args: ReadonlyArray<string>
	): Promise<
		| { readonly kind: 'exited'; readonly code: number; readonly stdout: string }
		| { readonly kind: 'signaled' | 'timed_out' | 'aborted'; readonly stdout: string }
	>;
}

/** The checkout's files, read by project-root-relative path — a gro `FsReadDir` is one. */
export interface StampReads {
	exists(path: string): Promise<boolean>;
	read_text(path: string): Promise<string>;
}

/** The checkout's files, read and written by project-root-relative path — a gro `FsDir` is
 * one. */
export interface StampWrites extends StampReads {
	write(path: string, data: string): Promise<unknown>;
	mkdir(path: string, options: { recursive: true }): Promise<void>;
}

/**
 * The modules whose per-file filters decide what a stamped corpus view yields: the
 * loader (every entry-level skip, extension set and exclusion) for both, and for
 * the `conformance` view the validity filter it applies to the Prettier suites. A
 * stamp keyed on checkouts and the entry list alone reads fresh across a filter
 * change — the Prettier filter moved `CSS_REJECTS_PIN` 229 → 207 with no stamped
 * input moving, and only the coverage run's own grade of that count caught it.
 * Listed by hand, so filter logic that moves to a new module must join its view's
 * list.
 */
const CORPUS_FILTER_MODULES: Record<'conformance' | 'perf', string[]> = {
	conformance: ['benches/js/lib/prettier_fixtures.ts', 'benches/js/lib/corpus.ts'],
	perf: ['benches/js/lib/corpus.ts']
};

/**
 * A digest of `view`'s {@link CORPUS_FILTER_MODULES} by source text, for a
 * view-loading grade's stamp (`filters`). A comment edit re-harvests too — the
 * price of not modelling which lines are filter logic, paid in seconds per harvest.
 */
export async function corpus_filter_fingerprint(
	fs: StampReads,
	view: 'conformance' | 'perf'
): Promise<string> {
	const hash = createHash('sha1');
	for (const path of CORPUS_FILTER_MODULES[view]) {
		hash
			.update(path)
			.update('\0')
			.update(await fs.read_text(path))
			.update('\0');
	}
	return hash.digest('hex').slice(0, 16);
}

/** `HEAD` commit of a checkout, or null when it isn't a git repo / git fails. */
export function git_head(git: StampGit, repo: string): Promise<string | null> {
	return git_rev_parse(git, repo, 'HEAD');
}

/** The object id a stamp records for `input` — its HEAD, or its subtree's id at HEAD. */
export function checkout_hash(git: StampGit, input: CheckoutInput): Promise<string | null> {
	return typeof input === 'string'
		? git_rev_parse(git, input, 'HEAD')
		: git_rev_parse(git, input.path, `HEAD:${input.tree}`);
}

/** How a message names a checkout input: the path, plus the subtree for a `tree` input. */
export function checkout_label(input: CheckoutInput): string {
	return typeof input === 'string' ? input : `${input.path}:${input.tree}`;
}

/**
 * `git rev-parse <rev>` in a checkout, or null when it isn't a git repo / git fails (one that
 * can't start included). `repo` is relative to git's working directory, the project root.
 */
async function git_rev_parse(git: StampGit, repo: string, rev: string): Promise<string | null> {
	try {
		const out = await git.out(['-C', repo, 'rev-parse', rev]);
		if (out.kind !== 'exited' || out.code !== 0) return null;
		return out.stdout.trim();
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
 * log / skip decision around it (skip is an exit 0 or a `return`, and the
 * log wording differs per harvest).
 */
export async function harvest_up_to_date(
	fs: StampReads,
	stamp_path: string,
	inputs: StampInputs,
	caches: string[]
): Promise<boolean> {
	for (const path of caches) {
		let present: boolean;
		try {
			present = await fs.exists(path);
		} catch {
			present = false;
		}
		if (!present) return false; // absent cache → harvest
	}
	return stamp_fresh(fs, stamp_path, inputs);
}

/** Whether the stamp at `path` records exactly `inputs`. */
async function stamp_fresh(fs: StampReads, path: string, inputs: StampInputs): Promise<boolean> {
	try {
		const recorded = JSON.parse(await fs.read_text(path)) as StampInputs;
		const keys = Object.keys(inputs);
		return (
			keys.length === Object.keys(recorded).length && keys.every((k) => recorded[k] === inputs[k])
		);
	} catch {
		return false;
	}
}

/** Record `inputs` at `path` — call only after the harvest + its pin check succeed. */
export async function write_stamp(
	fs: StampWrites,
	path: string,
	inputs: StampInputs
): Promise<void> {
	await fs.write(path, JSON.stringify(inputs, null, '\t') + '\n');
}

/**
 * Write a harvest's path-list cache, then its stamp — only once the list's exact
 * pin holds, so a wrong cache never replaces a good one. A mismatch is printed and
 * returns null, on which the caller exits 1 however the harvest was invoked:
 * `--if-present` tolerates a MISSING input, never a moved count. `stamp` is null when a
 * checkout has no commit to record.
 * @returns the cache's path as given, or null on a pin mismatch
 */
export async function write_pinned_path_cache(
	fs: StampWrites,
	options: {
		cache: string;
		paths: string[];
		pin: number;
		/** What the paths are, for the mismatch message (`143 rejects ≠ pinned 142`). */
		what: string;
		stamp: { path: string; inputs: StampInputs } | null;
	}
): Promise<string | null> {
	const { cache, paths, pin, what, stamp } = options;
	if (paths.length !== pin) {
		console.error(
			`FAIL: pinned count mismatch — ${paths.length} ${what} ≠ pinned ${pin}; cache not ` +
				'written. If the move is deliberate (suite refresh), re-pin in lib/gate_counts.ts.'
		);
		return null;
	}
	await fs.mkdir(dirname(cache), { recursive: true });
	await fs.write(cache, JSON.stringify(paths, null, '\t') + '\n');
	if (stamp) await write_stamp(fs, stamp.path, stamp.inputs);
	return cache;
}

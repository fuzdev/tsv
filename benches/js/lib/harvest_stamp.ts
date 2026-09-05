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
 * the script or, for test262, in the Rust runner). Stamps live in
 * `benches/js/.cache` (gitignored); `deno task bench:clean` wipes them with
 * the caches.
 *
 * Deno-only (like the harvests themselves — `git` runs via `Deno.Command`,
 * needing `--allow-run=git`).
 */

import { readFile, stat, writeFile } from 'node:fs/promises';

import { CORPORA_ROOT, CORPORA_TREE } from './corpora.ts';

export type StampInputs = Record<string, string | number | null>;

/**
 * A checkout input a stamp records: the checkout's HEAD commit (a path), or — when
 * only one subtree's bytes feed the grade — that subtree's object id at HEAD
 * (`git rev-parse HEAD:<tree>`), so a commit that touches nothing under the subtree
 * leaves the stamp fresh. The corpora snapshot is the one `tree` input today.
 */
export type CheckoutInput = string | { path: string; tree: string };

/** One stamped grade: where its stamp lives and which stamp keys record which checkout input. */
export interface HarvestStamp {
	/** Project-root-relative stamp path (under the gitignored `benches/js/.cache`). */
	path: string;
	/** The `deno task` that writes it — the remedy a stale-stamp message names. */
	task: string;
	/** Stamp key → the checkout input (`checkout_hash`) that key records. */
	checkouts: Record<string, CheckoutInput>;
}

/**
 * Every stamp, by grade name: the four suite harvests, the CSS reject pin — which
 * harvests nothing but is graded and stamped the same way
 * (`diagnostics/css_over_acceptance.ts --pin-only`) — and the svelte-styles harvest
 * over the real-code snapshot. Each script reads its own
 * `path` from here and `scripts/doctor.ts` walks the table to report a stamp whose
 * recorded checkout id no longer matches the checkout (its HEAD, or the pinned
 * subtree's id) — one place, so a renamed stamp or a new checkout input can't leave
 * the doctor reading a file nothing writes. The `checkouts` keys are the stamp's OWN key names; a doctor probe that
 * finds a listed key absent from the stamp reports that rather than guessing.
 *
 * `as const satisfies` rather than a bare `Record` annotation: the six scripts read
 * their own entry by key, and under a `Record<string, …>` a renamed key still
 * typechecks and fails at runtime — which is the drift this table exists to prevent.
 */
export const HARVEST_STAMPS = {
	'wpt-css': {
		path: 'benches/js/.cache/wpt_css.stamp.json',
		task: 'bench:harvest:wpt',
		// `../wpt/css`, not `../wpt`, and the difference is deliberate even though the
		// two report the same SHA: this harvest stamps whatever `--source` it actually
		// read, whose default is that subtree. (`css-rejects` below names `../wpt`
		// because it consumes the repo through this harvest's CACHE and has no
		// `--source` of its own.)
		checkouts: { source_commit: '../wpt/css' }
	},
	test262: {
		path: 'benches/js/.cache/test262.stamp.json',
		task: 'bench:harvest:test262',
		checkouts: { source_commit: '../test262' }
	},
	ts_repo: {
		path: 'benches/js/.cache/ts_repo.stamp.json',
		task: 'bench:harvest:ts-repo',
		checkouts: { source_commit: '../typescript' }
	},
	'svelte-rejects': {
		path: 'benches/js/.cache/svelte_parse_rejects.stamp.json',
		task: 'bench:harvest:svelte-rejects',
		// Three, because the Svelte-language conformance corpus is three suites:
		// ../svelte's tests plus both prettier suites' `.html` files, which the
		// loader reads as Svelte (98 / 40 / 7 of the pinned 145 rejects).
		checkouts: {
			svelte_commit: '../svelte',
			prettier_commit: '../prettier',
			prettier_plugin_svelte_commit: '../prettier-plugin-svelte'
		}
	},
	'css-rejects': {
		path: 'benches/js/.cache/css_rejects.stamp.json',
		task: 'css:over-acceptance:pin',
		checkouts: {
			svelte_commit: '../svelte',
			prettier_commit: '../prettier',
			wpt_commit: '../wpt'
		}
	},
	// The perf view's `.svelte` files all come from the `../corpora` snapshot, so its
	// `collections/` tree id is the harvest's checkout input (the harvest also stamps
	// the pinned count and the perf view's entry list, which no checkout records).
	'svelte-styles': {
		path: 'benches/js/.cache/svelte_styles.stamp.json',
		task: 'bench:harvest:svelte-styles',
		checkouts: { corpora_tree: { path: CORPORA_ROOT, tree: CORPORA_TREE } }
	}
} as const satisfies Record<string, HarvestStamp>;

/** `HEAD` commit of a checkout, or null when it isn't a git repo / git fails. */
export function git_head(repo: string): string | null {
	return git_rev_parse(repo, 'HEAD');
}

/** The object id a stamp records for `input` — its HEAD, or its subtree's id at HEAD. */
export function checkout_hash(input: CheckoutInput): string | null {
	return typeof input === 'string'
		? git_rev_parse(input, 'HEAD')
		: git_rev_parse(input.path, `HEAD:${input.tree}`);
}

/** How a message names a checkout input: the path, plus the subtree for a `tree` input. */
export function checkout_label(input: CheckoutInput): string {
	return typeof input === 'string' ? input : `${input.path}:${input.tree}`;
}

/** `git rev-parse <rev>` in a checkout, or null when it isn't a git repo / git fails. */
function git_rev_parse(repo: string, rev: string): string | null {
	try {
		const out = new Deno.Command('git', {
			args: ['-C', repo, 'rev-parse', rev],
			stdout: 'piped',
			stderr: 'null'
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
	caches: string[]
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
async function stamp_fresh(path: string, inputs: StampInputs): Promise<boolean> {
	try {
		const recorded = JSON.parse(await readFile(path, 'utf8')) as StampInputs;
		const keys = Object.keys(inputs);
		return (
			keys.length === Object.keys(recorded).length && keys.every((k) => recorded[k] === inputs[k])
		);
	} catch {
		return false;
	}
}

/** Record `inputs` at `path` — call only after the harvest + its pin check succeed. */
export async function write_stamp(path: string, inputs: StampInputs): Promise<void> {
	await writeFile(path, JSON.stringify(inputs, null, '\t') + '\n');
}

/**
 * Every harvest freshness stamp, by grade name: where each lives, the task that writes it, and
 * which checkout input each stamp key records. The stamps themselves are `harvest_stamp.ts`'s;
 * this table is apart from it so a reader of the table (`corpus_repos.ts`, in Bun's bench
 * closure; `scripts/doctor.ts`) loads data, never the stamp logic's file reads and `git`.
 * Its one import is the corpora snapshot's location.
 *
 * @module
 */

import { CORPORA_ROOT, CORPORA_TREE } from './corpora.ts';

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
	/** Stamp key → the checkout input (`checkout_hash` in `harvest_stamp.ts`) that key records. */
	checkouts: Record<string, CheckoutInput>;
}

/**
 * Every stamp, by grade name: the five suite harvests, the CSS reject pin — which
 * harvests nothing but is graded and stamped the same way
 * (`diagnostics/css_over_acceptance.ts --pin-only`) — and the svelte-styles harvest
 * over the real-code snapshot. Each script reads its own
 * `path` from here and `scripts/doctor.ts` walks the table to report a stamp whose
 * recorded checkout id no longer matches the checkout (its HEAD, or the pinned
 * subtree's id) — one place, so a renamed stamp or a new checkout input can't leave
 * the doctor reading a file nothing writes. The `checkouts` keys are the stamp's OWN key names; a doctor probe that
 * finds a listed key absent from the stamp reports that rather than guessing.
 *
 * `as const satisfies` rather than a bare `Record` annotation: the seven scripts read
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
		// loader reads as Svelte (the split of the pinned rejects is in `SVELTE_REJECTS_PIN`'s doc).
		checkouts: {
			svelte_commit: '../svelte',
			prettier_commit: '../prettier',
			prettier_plugin_svelte_commit: '../prettier-plugin-svelte'
		}
	},
	// The Prettier `.js` fixtures Prettier's own babel parser reads as JSX — one
	// checkout, since only `../prettier/tests/format/js` holds JS-language
	// fixtures the conformance view keeps (the other suites' `.js` were their spec
	// files, dropped by the validity filter).
	'prettier-jsx': {
		path: 'benches/js/.cache/prettier_jsx_files.stamp.json',
		task: 'bench:harvest:prettier-jsx',
		checkouts: { prettier_commit: '../prettier' }
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

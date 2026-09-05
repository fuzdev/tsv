/**
 * Discovery parity over the real-code snapshot: does `tsv format --list` over
 * `../corpora/collections` name exactly the files the snapshot holds in the extensions
 * tsv formats?
 *
 * ## What it proves
 *
 * The corpus every real-code gate and bench reads is DEFINED by the snapshot's committed
 * tree (its materializer lists tracked files at the pinned commits), not by tsv's own
 * discovery — deliberately, so a tsv discovery bug cannot silently shrink every consumer's
 * corpus. This leg is what makes that separation observable: it asks the production
 * discovery (`tsv_cli`'s walk, ignore-file and safety-net handling included) for the
 * in-scope files and compares them with `git ls-files` over the same tree, filtered to the
 * extensions tsv formats. The two lists must be identical:
 *
 * - a file the snapshot holds that tsv does NOT list is a discovery finding — a prune
 *   that fires on real code (a directory name a safety net or the build-output heuristic
 *   claims), or an extension tsv stopped formatting;
 * - a file tsv lists that the snapshot does NOT hold is a bookkeeping finding — an
 *   extension tsv formats that `TSV_EXTENSIONS` below does not name (update it), or an
 *   untracked file in the checkout.
 *
 * Either direction is surfaced rather than baked into the next re-pin. The extension
 * list is restated here on purpose: reading it from the Rust side would make the probe
 * agree with tsv by construction, which is exactly what it must not do.
 *
 * ## Why absence is a warning, not a failure
 *
 * Same posture as `roundtrip:audit:prettier`: `deno task check` must run on a bare
 * checkout (the CI `check` job has no sibling checkouts), so `../corpora` is read
 * opportunistically — present, and the leg gates; absent, and it prints a loud NOT RUN
 * line and exits 0. It is an invariant check, not a count pin, so a finding fails
 * wherever it occurs and a missing checkout costs only coverage.
 *
 * A DIRTY checkout is refused rather than graded: `git ls-files` describes the committed
 * tree while `tsv format --list` walks the disk, so a local modification would read as a
 * tsv finding. `deno task doctor` reports the same state.
 *
 * Cost is ~0.1 s (pure Rust, ~6,700 files — every collection the snapshot vendors, not
 * only the ones the bench views read) on the `--profile corpus` `tsv_cli` binary the
 * `format:audit` leg has already built.
 */

import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { relative, resolve } from 'node:path';

import {
	CORPORA_COLLECTIONS,
	CORPORA_ROOT,
	CORPORA_TREE,
	CORPORA_URL
} from '../benches/js/lib/corpora.ts';

/**
 * The extensions `tsv format` discovers — the JS/TS family, Svelte, CSS. Restated rather
 * than imported from the Rust side (see the module doc): a mismatch in either direction
 * is the finding. The snapshot also carries `.html`, `.md` and `.mdz` for other
 * consumers; tsv formats none of them, so they are expected NOT to be listed.
 */
const TSV_EXTENSIONS = ['ts', 'mts', 'cts', 'js', 'mjs', 'cjs', 'svelte', 'css'];

const log = (...args: unknown[]) => console.error(...args);

const has_tsv_extension = (path: string): boolean => {
	const dot = path.lastIndexOf('.');
	return dot > path.lastIndexOf('/') && TSV_EXTENSIONS.includes(path.slice(dot + 1));
};

/** Runs `git -C ../corpora <args>`; the trimmed stdout, or `null` on a non-zero exit. */
function git(args: string[]): string | null {
	const { status, stdout } = spawnSync('git', ['-C', CORPORA_ROOT, ...args], {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'inherit']
	});
	return status === 0 ? stdout : null;
}

function run(): never {
	const tree = CORPORA_COLLECTIONS;
	if (!existsSync(tree)) {
		log(`⚠ discovery:audit NOT RUN — no ${CORPORA_ROOT} checkout.`);
		log("  This leg checks tsv's discovery against the real-code snapshot's committed file list.");
		log(`  Clone it beside this repo to gate it locally: git clone ${CORPORA_URL} ${CORPORA_ROOT}`);
		process.exit(0);
	}

	const dirty = git(['status', '--porcelain', '--', CORPORA_TREE]);
	if (dirty === null) {
		log(
			`Error: ${CORPORA_ROOT} is not a git checkout — the snapshot's file list is its committed tree.`
		);
		process.exit(1);
	}
	if (dirty.trim() !== '') {
		log(
			`Error: ${tree} has local modifications — restore it before grading ` +
				`(git -C ${CORPORA_ROOT} checkout -- ${CORPORA_TREE}):\n${dirty.trimEnd()}`
		);
		process.exit(1);
	}

	// The committed tree, filtered to what tsv formats — paths relative to the corpora root.
	const listed = git(['ls-files', '-z', '--', CORPORA_TREE]);
	if (listed === null) process.exit(1);
	const expected = new Set(listed.split('\0').filter((p) => p !== '' && has_tsv_extension(p)));
	if (expected.size === 0) {
		log(`Error: ${tree} holds no file in tsv's extensions — an empty snapshot grades nothing.`);
		process.exit(1);
	}

	// The production discovery, on the binary `format:audit` built earlier in `check`.
	const list = spawnSync(
		'cargo',
		['run', '--profile', 'corpus', '-p', 'tsv_cli', '--quiet', '--', 'format', '--list', tree],
		{ encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }
	);
	if (list.status !== 0) {
		log(`Error: tsv format --list exited ${list.status}.`);
		process.exit(list.status ?? 1);
	}
	const root = resolve(CORPORA_ROOT);
	const actual = new Set(
		list.stdout
			.split('\n')
			.filter((line) => line !== '')
			.map((line) => relative(root, resolve(line)))
	);

	const not_listed = [...expected].filter((p) => !actual.has(p)).sort();
	const not_in_tree = [...actual].filter((p) => !expected.has(p)).sort();

	if (not_listed.length === 0 && not_in_tree.length === 0) {
		log(
			`discovery:audit OK — tsv format --list names all ${actual.size} snapshot files in its extensions`
		);
		process.exit(0);
	}

	const show = (paths: string[]): string =>
		paths
			.slice(0, 40)
			.map((p) => `    ${p}`)
			.join('\n') + (paths.length > 40 ? `\n    … and ${paths.length - 40} more` : '');
	if (not_listed.length > 0) {
		log(
			`FAIL: ${not_listed.length} snapshot file(s) tsv format --list did NOT name — a discovery prune ` +
				'firing on real code, or an extension tsv stopped formatting:\n' +
				show(not_listed)
		);
	}
	if (not_in_tree.length > 0) {
		log(
			`FAIL: ${not_in_tree.length} file(s) tsv format --list named that the committed snapshot does NOT ` +
				'hold in TSV_EXTENSIONS — an extension this probe must learn, or an untracked file:\n' +
				show(not_in_tree)
		);
	}
	process.exit(1);
}

run();

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
 * Same posture as `roundtrip:audit:prettier`: `npm run check` must run on a bare
 * checkout (the CI `check` job has no sibling checkouts), so `../corpora` is read
 * opportunistically — present, and the leg gates; absent, and it prints a loud NOT RUN
 * line and exits 0. It is an invariant check, not a count pin, so a finding fails
 * wherever it occurs and a missing checkout costs only coverage. A checkout that is
 * present without the snapshot's `collections/` is a broken one rather than a machine
 * without one: it says so and exits 1.
 *
 * A DIRTY checkout is refused rather than graded: `git ls-files` describes the committed
 * tree while `tsv format --list` walks the disk, so a local modification would read as a
 * tsv finding. `npm run doctor` reports the same state.
 *
 * Cost is ~0.1 s (pure Rust, ~6,700 files — every collection the snapshot vendors, not
 * only the ones the bench views read) on the `--profile corpus` `tsv` binary of the shared
 * build world (docs/tasks.md §`build:check`), which the `format:audit` leg has already built.
 *
 * ## Reach
 *
 * The `../corpora` checkout, read-only, opened by its path, so one that is a symlink to a
 * directory elsewhere is followed (whether the snapshot is there); `git`, which reads the
 * snapshot's status and file list in it; and the checkout's `target/corpus/tsv`, run in the
 * checkout (`DiscoveryAuditDeps`, built by `scripts/roots/discovery_audit.ts`).
 */

import { relative, resolve } from 'node:path';

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import {
	CORPORA_COLLECTIONS,
	CORPORA_ROOT,
	CORPORA_TREE,
	CORPORA_URL
} from '../benches/js/lib/corpora.ts';
import { BROKEN_CHECKOUT } from './sibling_text.ts';

/**
 * The extensions `tsv format` discovers — the JS/TS family, Svelte, CSS. Restated rather
 * than imported from the Rust side (see the module doc): a mismatch in either direction
 * is the finding. The snapshot also carries `.html`, `.md` and `.mdz` for other
 * consumers; tsv formats none of them, so they are expected NOT to be listed.
 */
const TSV_EXTENSIONS = ['ts', 'mts', 'cts', 'js', 'mjs', 'cjs', 'svelte', 'css'];

/**
 * The `tsv` binary of the shared `--profile corpus` build world (docs/tasks.md §`build:check`),
 * which the `discovery:audit` task builds before running this.
 */
export const DISCOVERY_TSV_BIN = 'target/corpus/tsv';

/** What `run_discovery_audit` reaches. */
export interface DiscoveryAuditDeps {
	/** The checkout, `tsv`'s working directory. */
	checkout: FsReadDir;
	/**
	 * The `../corpora` checkout (`CORPORA_ROOT`): its handle, `git`'s working directory, or
	 * `null` when it is absent, and whether a path beneath it exists.
	 */
	corpora: { open(): Promise<FsReadDir | null>; exists(path: string): Promise<boolean> };
	/**
	 * `git`, its stdout captured and its stderr passed through, or `null` when there is none,
	 * which grades a present snapshot as no git checkout.
	 */
	git: SpawnProgram | null;
	/** The checkout's `target/corpus/tsv` (`DISCOVERY_TSV_BIN`). */
	tsv: SpawnProgram;
}

const log = (...args: unknown[]) => console.error(...args);

const has_tsv_extension = (path: string): boolean => {
	const dot = path.lastIndexOf('.');
	// `dot > slash + 1`, not `> slash`: a bare dotfile (`.ts`) is a stem with no
	// extension to `Path::extension`, which is what `is_formattable` reads
	return dot > path.lastIndexOf('/') + 1 && TSV_EXTENSIONS.includes(path.slice(dot + 1));
};

/**
 * Runs `program <args>` in `cwd` with its stderr passed through; its exit code (`null` when a
 * signal ended it) and its stdout.
 */
async function run_capturing_stdout(
	program: SpawnProgram,
	args: string[],
	cwd: FsReadDir
): Promise<{ code: number | null; stdout: string }> {
	const child = await program.start(args, { cwd, stdio: ['ignore', 'pipe', 'inherit'] });
	const chunks: Uint8Array[] = [];
	let length = 0;
	for await (const chunk of child.stdout!) {
		chunks.push(chunk);
		length += chunk.length;
	}
	const bytes = new Uint8Array(length);
	let at = 0;
	for (const chunk of chunks) {
		bytes.set(chunk, at);
		at += chunk.length;
	}
	const result = await child.closed;
	return {
		code: result.kind === 'exited' ? result.code : null,
		stdout: new TextDecoder().decode(bytes)
	};
}

/** Runs `git <args>` in `../corpora`; its stdout, or `null` on a non-zero exit or no git. */
async function git(
	program: SpawnProgram | null,
	corpora: FsReadDir,
	args: string[]
): Promise<string | null> {
	if (program === null) return null;
	const { code, stdout } = await run_capturing_stdout(program, args, corpora);
	return code === 0 ? stdout : null;
}

/**
 * The audit (module doc).
 * @returns the exit code: `0` when the two lists agree or no checkout is present, else `1`
 * (or `tsv`'s own non-zero code)
 */
export async function run_discovery_audit(deps: DiscoveryAuditDeps): Promise<number> {
	const tree = CORPORA_COLLECTIONS;
	const corpora = await deps.corpora.open();
	if (corpora === null) {
		log(`⚠ discovery:audit NOT RUN — no ${CORPORA_ROOT} checkout.`);
		log("  This leg checks tsv's discovery against the real-code snapshot's committed file list.");
		log(`  Clone it beside this repo to gate it locally: git clone ${CORPORA_URL} ${CORPORA_ROOT}`);
		return 0;
	}
	if (!(await deps.corpora.exists(CORPORA_TREE))) {
		log(
			`Error: a ${CORPORA_ROOT} checkout is present but holds no ${CORPORA_TREE}/, the snapshot this leg grades — ` +
				`${BROKEN_CHECKOUT} to skip the leg.`
		);
		return 1;
	}

	const dirty = await git(deps.git, corpora, ['status', '--porcelain', '--', CORPORA_TREE]);
	if (dirty === null) {
		log(
			`Error: ${CORPORA_ROOT} is not a git checkout — the snapshot's file list is its committed tree.`
		);
		return 1;
	}
	if (dirty.trim() !== '') {
		log(
			`Error: ${tree} has local modifications — restore it before grading ` +
				`(git -C ${CORPORA_ROOT} checkout -- ${CORPORA_TREE}):\n${dirty.trimEnd()}`
		);
		return 1;
	}

	// The committed tree, filtered to what tsv formats — paths relative to the corpora root.
	const listed = await git(deps.git, corpora, ['ls-files', '-z', '--', CORPORA_TREE]);
	if (listed === null) return 1;
	const expected = new Set(listed.split('\0').filter((p) => p !== '' && has_tsv_extension(p)));
	if (expected.size === 0) {
		log(`Error: ${tree} holds no file in tsv's extensions — an empty snapshot grades nothing.`);
		return 1;
	}

	// The production discovery, on the `--profile corpus` `tsv` binary. Exec'd by PATH rather
	// than through `cargo run`: that form selects `-p tsv_cli` alone, which resolves features
	// without `_internal_audits` and so rebuilds the whole stack into a second corpus-profile world (see
	// docs/tasks.md §`build:check`). The `discovery:audit` task runs `build:check` ahead of this,
	// so the binary is fresh on a standalone run too.
	const list = await run_capturing_stdout(deps.tsv, ['format', '--list', tree], deps.checkout);
	const list_status = list.code;
	if (list_status !== 0) {
		log(`Error: tsv format --list exited ${list_status}.`);
		return list_status ?? 1;
	}
	const root = resolve(deps.checkout.path, CORPORA_ROOT);
	const actual = new Set(
		list.stdout
			.split('\n')
			.filter((line) => line !== '')
			.map((line) => relative(root, resolve(deps.checkout.path, line)))
	);

	const not_listed = [...expected].filter((p) => !actual.has(p)).sort();
	const not_in_tree = [...actual].filter((p) => !expected.has(p)).sort();

	if (not_listed.length === 0 && not_in_tree.length === 0) {
		log(
			`discovery:audit OK — tsv format --list names all ${actual.size} snapshot files in its extensions`
		);
		return 0;
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
	return 1;
}

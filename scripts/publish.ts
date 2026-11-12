/**
 * Publish script for the tsv npm packages.
 *
 * The WASM packages only: the N-API set publishes from `.github/workflows/release.yml`
 * (triggered by the tag this script pushes), and the crates from
 * `scripts/publish_crates.ts`, run after that workflow's GitHub Release. The bump here
 * writes every version all three read (the workspace version and its `=x.y.z` sibling pins).
 *
 * Dry-run by default — runs all validation but does not mutate the workspace.
 * Pass `--wetrun` to bump, build, validate, publish to npm, and finalize git
 * (commit + tag + push). The build step covers the npm packages AND the deno
 * bundles — the deno bundles aren't published, but artifact validation checks
 * every built bundle, so they're rebuilt here to never gate on stale artifacts.
 *
 * Version source of truth: `Cargo.toml [workspace.package] version` (read
 * directly by wasm-pack). The root package.json is private and carries no version;
 * there are no changesets — `--bump` edits Cargo.toml and converts CHANGELOG.md's
 * `## Unreleased` section into the new version's section (the grammar is `scripts/release/changelog.ts`;
 * that stamped section is what the tag's GitHub Release reads back as its body,
 * via `scripts/release_notes.ts` in the release workflow). What a run releases
 * — fresh or resumed, and every reason a fresh one cannot — is decided once by
 * `scripts/release/plan.ts`; the dry run warns on exactly what a wetrun refuses.
 *
 * Usage (`deno task publish`, a `deno.json` task, so no npm lifecycle can start it):
 *   deno task publish --bump patch          # dry-run (validate everything, preview the bump)
 *   deno task publish --wetrun --bump patch  # bump + check + conformance:all + audit:corpus + build + validate + publish + git
 *   deno task publish --wetrun               # resume a failed wetrun (sentinel retry only)
 *
 * Flags:
 *   --wetrun         actually mutate: bump, publish, commit, tag, push
 *   --bump <level>   patch | minor | major — how to bump the version. Required,
 *                    and must match the CHANGELOG `## Unreleased` `<!-- bump:
 *                    <level> -->` marker (also required). A fresh wetrun needs
 *                    both, plus a non-empty `## Unreleased` section; a sentinel
 *                    retry runs without either.
 *   --no-check       skip `deno task check` (with the depth contract's shipped-build
 *                    and WASM legs, `test:depth:napi` and Step 6's `test:depth:wasm`),
 *                    the Step 3b conformance gates, AND the
 *                    Step 3c corpus audit (faster retries; also the only way to --wetrun
 *                    on a machine missing the Step 3b/3c oracle checkouts — a missing
 *                    oracle otherwise FAILS a wetrun — or without bun, which Step 6's
 *                    Bun leg otherwise requires of a wetrun). It does not waive the
 *                    benches/js TypeScript install, which Step 6's declaration check needs
 *   --no-git         skip the git commit + tag + push finalization
 *
 * Preflight (Step 1), before anything is mutated or built: the release-state checks —
 * the plan's problems, a dirty tree, a branch other than main, a HEAD that is not
 * exactly origin/main, a `v<version>` tag that already exists locally or on origin,
 * a failing `npm whoami`, and npm's `dry-run` setting anything but `false` — refuse a wetrun
 * and warn a dry run; a missing tool, a failed registry lookup, and a fresh release whose
 * version is already on npm refuse both.
 *
 * A wetrun counts a package as published only once the registry lists its version
 * (`PUBLISH_CONFIRM_READS`), since an `npm publish` that exits 0 may have published nothing;
 * one never listed stops the run before git, resumable.
 *
 * The checkout is fingerprinted (`git_checkout_fingerprint`: HEAD, status, and the
 * tracked diff's content) after the bump and again after Step 6: a checkout that moved
 * while the gates ran refuses the publish, since the packages would describe no commit.
 *
 * Retry: a failed wetrun leaves the bump in place plus a sentinel file
 * (`scripts/release/sentinel.ts`); re-run `deno task publish --wetrun` and it resumes
 * from the bumped version (skipping the bump; a passed `--bump` is ignored with a
 * warning). A resume accepts a tree dirty with only the release files, one release
 * commit ahead of origin/main, and a tag on HEAD. Every mutating step is idempotent —
 * already-published packages are skipped, and the git finalize no-ops on an existing
 * commit/tag — so the sentinel covers the run through Step 8 and is removed only once
 * the push lands. A resume whose packages are all already on npm skips straight to
 * the git finalize.
 *
 * Its reach (`PublishDeps`, built by `scripts/roots/publish_deps.ts` for the entry
 * `scripts/roots/publish.ts`): the checkout, read and written (the bump's files and the
 * sentinel; cargo writes the lockfile), every program's working directory, and every path
 * read against it; whether the oracle checkouts beside it exist; and one handle per program
 * and environment (`PublishPrograms`).
 *
 * @module
 */

import type { FsDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { gzip_size } from './gzip_size.ts';
import {
	bump_cargo_version,
	bump_changelog,
	bump_lockfile,
	release_commit_message,
	RELEASE_PATHS
} from './release/bump.ts';
import { CHANGELOG_PATH } from './release/changelog.ts';
import {
	git_checkout_fingerprint,
	git_commit_paths,
	git_current_branch,
	git_fetch_and_compare_origin_main,
	git_head_subject,
	git_local_tag_commit,
	git_push_main_with_tags,
	git_remote_tag_commit,
	git_rev,
	git_status_line_path,
	git_status_lines,
	git_tag_annotated,
	type ReleaseGit
} from './release/git.ts';
import {
	npm_dry_run_setting,
	npm_error_is_republish,
	npm_is_published,
	npm_whoami,
	type ReleaseNpm
} from './release/npm.ts';
import { release_plan } from './release/plan.ts';
import { program_capture, program_stream } from './release/program.ts';
import { sentinel_read, sentinel_remove, sentinel_write } from './release/sentinel.ts';
import {
	type BumpLevel,
	CARGO_PATH,
	cargo_workspace_version,
	cargo_workspace_version_set,
	is_bump_level,
	version_tag
} from './release/version.ts';
import {
	SIDECAR_DIR,
	SIDECAR_INSTALL_COMMAND,
	SIDECAR_LOCK_PATH,
	sidecar_install_state
} from './sidecar_install.ts';
import { format_size } from './size.ts';
import { TYPECHECK_INSTALL } from './typecheck_install.ts';

/**
 * The programs `run_publish` runs, one handle per environment, each `null` when it isn't on
 * `PATH`, which fails only the call that needs it (as a missing command did).
 */
export interface PublishPrograms {
	/** `deno`, for the `deno task` gates and builds, with the owner's whole environment. */
	deno: SpawnProgram | null;
	/** `deno` for Step 4's `build:packages`: the same environment with `TSV_BUILD_FORCE=1`
	 * added, so no bundle is skipped as fresh. */
	deno_build: SpawnProgram | null;
	/** `node`, for the version probe and the loader staging. */
	node: SpawnProgram | null;
	/** `node` for Step 6's artifact tests of the package staged at `pkg_dir`, with the owner's
	 * whole environment and `PKG_DIR` set to it. */
	node_test: (pkg_dir: string) => SpawnProgram | null;
	/** `npm`, with the owner's whole environment: its config, auth, and registry. */
	npm: SpawnProgram | null;
	/** `git`, reading the checkout and reaching `origin` (`ReleaseGit`). */
	git: Omit<ReleaseGit, 'cwd'>;
	/** `cargo`, for the lockfile update. */
	cargo: SpawnProgram | null;
	/** `wasm-pack` and `bun`, for their version probes. */
	wasm_pack: SpawnProgram | null;
	bun: SpawnProgram | null;
	/** `gzip`, for the summary's gzipped sizes; a missing one fails the summary, after the
	 * publish, as it always has. */
	gzip: SpawnProgram | null;
}

/** What `run_publish` reaches. */
export interface PublishDeps {
	/** The checkout: what the run reads and writes, and every program's working directory. */
	checkout: FsDir;
	/** Whether `path`, read against the checkout and climbing to the checkouts beside it
	 * (`../svelte/…`), exists, following symlinks. A missing path is `false`; any other
	 * failure throws. */
	exists: (path: string) => Promise<boolean>;
	/** The real-code snapshot's paths, `CORPORA_ROOT` and `CORPORA_COLLECTIONS`
	 * (`benches/js/lib/corpora.ts`), handed in because that module reads files itself. */
	corpora: { root: string; collections: string };
	programs: PublishPrograms;
	/** Writes a line to stdout. */
	print: (line: string) => void;
	/** Writes a line to stderr. */
	print_error: (line: string) => void;
	/** Waits `ms` milliseconds: the pause between the registry reads that confirm a publish. */
	sleep: (ms: number) => Promise<void>;
}

const USAGE =
	'Usage: deno task publish [--wetrun] [--bump patch|minor|major] [--no-check] [--no-git]';

/** The oldest node Step 6 runs on — the first with native TypeScript type stripping
 * unflagged, which `node --test` needs to execute the `.ts` test files directly. */
const NODE_MIN = [22, 18] as const;

/** The published packages, in publish order. */
export const PUBLISH_PACKAGES: ReadonlyArray<{ label: string; dir: string }> = [
	{
		label: '@fuzdev/tsv-format-wasm',
		dir: 'crates/tsv_wasm/pkg/format/npm'
	},
	{
		label: '@fuzdev/tsv-parse-wasm',
		dir: 'crates/tsv_wasm/pkg/parse/npm'
	},
	{
		label: '@fuzdev/tsv-wasm',
		dir: 'crates/tsv_wasm/pkg/all/npm'
	}
];

/**
 * How long a `--wetrun` waits for the registry to list a version its `npm publish` just
 * published: `PUBLISH_CONFIRM_READS` reads of `npm view`, the first at once and the rest
 * `PUBLISH_CONFIRM_RETRY_MS` apart, five minutes in all. A version the registry never lists is
 * a publish that didn't happen (npm's dry-run setting turns it into a dry run that exits 0) or
 * a registry that is behind; either way the run stops before git, and a resume publishes the
 * version if it is still absent and skips it once it is listed.
 */
export const PUBLISH_CONFIRM_READS = 31;
export const PUBLISH_CONFIRM_RETRY_MS = 10_000;

/** Thrown by `exit` to end the run with `code`, which `run_publish` returns. */
class PublishExit extends Error {
	readonly code: number;
	constructor(code: number) {
		super(`exit ${code}`);
		this.code = code;
	}
}

/**
 * Publish the wasm packages, or dry-run it (module doc).
 * @param argv - the flags
 * @returns the exit code: `1` for a bad flag, a refusal, or a failed step, else `0`
 */
export async function run_publish(deps: PublishDeps, argv: ReadonlyArray<string>): Promise<number> {
	try {
		await publish(deps, argv);
		return 0;
	} catch (error) {
		if (error instanceof PublishExit) return error.code;
		throw error;
	}
}

async function publish(deps: PublishDeps, argv: ReadonlyArray<string>): Promise<void> {
	const { checkout, exists, programs, print, print_error } = deps;
	const git: ReleaseGit = { ...programs.git, cwd: checkout };
	const npm: ReleaseNpm = { program: programs.npm, cwd: checkout };
	const packages = PUBLISH_PACKAGES;
	// annotated, so a call narrows like a `throw` (control flow reads only annotated `never`s)
	const exit: (code: number) => never = (code) => {
		throw new PublishExit(code);
	};

	// A misspelled --wetrun fails safe (dry-run), but a misspelled --no-git or
	// --no-check fails open — reject anything unrecognized.
	let wetrun = false;
	let no_check = false;
	let no_git = false;
	let bump: BumpLevel | null = null;
	const unknown_args: Array<string> = [];
	for (let i = 0; i < argv.length; i++) {
		const arg = argv[i]!;
		if (arg === '--wetrun') wetrun = true;
		else if (arg === '--no-check') no_check = true;
		else if (arg === '--no-git') no_git = true;
		else if (arg === '--bump') {
			const level = argv[++i];
			if (!is_bump_level(level)) {
				print_error(
					`FAIL: --bump must be patch, minor, or major (got ${JSON.stringify(level ?? '')})`
				);
				exit(1);
			}
			bump = level;
		} else unknown_args.push(arg);
	}
	if (unknown_args.length > 0) {
		print_error(`FAIL: unknown argument(s): ${unknown_args.join(' ')}`);
		print_error(USAGE);
		exit(1);
	}

	/** Set once the bump is written: from then on a failure names the resume. */
	let bumped = false;

	const exit_resumable: () => never = () => {
		if (bumped) {
			print_error(
				'  The bump is in place — fix the cause and run `deno task publish --wetrun` to resume.'
			);
		}
		return exit(1);
	};

	/** Exit on a failure, naming the resume once the bump is in place. */
	const fail_resumable: (message: string) => never = (message) => {
		print_error(`  FAIL: ${message}`);
		return exit_resumable();
	};

	/** A tool's `--version` output, or `null` when it is not installed or fails. */
	const probe_version = async (program: SpawnProgram | null, name: string) => {
		const result = await program_capture(program, name, ['--version'], { cwd: checkout });
		return result.success ? result.out.trim() : null;
	};

	/** Run a long step with the terminal attached, in the checkout; exit on failure, printing
	 * `fail_hint` and, after the bump, the resume under the failure. */
	const run = async (
		label: string,
		program: SpawnProgram | null,
		name: string,
		args: ReadonlyArray<string>,
		options: { cwd?: FsDir; fail_hint?: string } = {}
	): Promise<void> => {
		const result = await program_stream(
			program,
			name,
			args,
			{ cwd: options.cwd ?? checkout },
			print_error
		);
		if (result.success) return;
		print_error(`\n  FAIL: ${label} (exit code ${result.code})`);
		if (options.fail_hint) print_error(options.fail_hint);
		exit_resumable();
	};

	print(`\n=== tsv publish ${wetrun ? '(wetrun)' : '(dry-run)'} ===\n`);

	// Step 1: Preflight checks

	print('=== Step 1: Preflight checks ===');

	const cargo_toml = await checkout.read_text(CARGO_PATH);
	const version_before = cargo_workspace_version(cargo_toml);
	if (version_before === null) {
		print_error('  FAIL: could not find [workspace.package] version in Cargo.toml');
		exit(1);
	}
	const plan = release_plan({
		version: version_before,
		bump,
		sentinel: await sentinel_read(checkout),
		// only a missing file reads as null; any other failure to read it throws
		changelog: await checkout.read_text_if_exists(CHANGELOG_PATH)
	});
	/** The version this run releases; null only when a fresh plan has no target, which a
	 * wetrun refuses. */
	const release_version = plan.kind === 'resume' ? plan.version : plan.target;
	const release_tag = release_version === null ? null : version_tag(release_version);

	if (plan.kind === 'resume') {
		print(`  Sentinel found at v${plan.version} — resuming that release`);
		for (const note of plan.notes) print_error(`  WARN: ${note}`);
	} else {
		print(
			`  Version: ${plan.version} -> ${plan.target ?? '?'}${plan.level ? ` (${plan.level})` : ''}`
		);
		if (plan.stale_sentinel !== null) {
			print_error(`  WARN: stale sentinel (v${plan.stale_sentinel}) — a fresh wetrun removes it`);
		}
	}

	// The release-state checks: a refusal under --wetrun and a warning in a dry run,
	// collected so one run names every reason. A missing tool or a failed registry
	// lookup refuses either way (`failures`).
	const failures: Array<string> = [];
	const refusals: Array<string> = [];
	/** How many refusals this dry run downgraded to warnings, so its last line can say a
	 * wetrun would refuse. */
	let warnings = 0;
	const refuse_or_warn = (message: string): void => {
		if (wetrun) refusals.push(message);
		else {
			warnings++;
			// the verdict goes on the first line, ahead of any listing the message carries
			const [first, ...rest] = message.split('\n');
			print_error([`  WARN: ${first} — a wetrun would FAIL`, ...rest].join('\n'));
		}
	};

	if (plan.kind === 'fresh') for (const problem of plan.problems) refuse_or_warn(problem);
	// the bump's own write, previewed: a sibling pin it cannot move refuses here, before the bump,
	// rather than at the lockfile update after it
	if (plan.kind === 'fresh' && plan.target !== null) {
		const set = cargo_workspace_version_set(cargo_toml, plan.target);
		if (!set.ok) refuse_or_warn(set.error);
	}

	const branch = await git_current_branch(git);
	if (branch === null) refuse_or_warn('git branch failed — is this a git repository?');
	else if (branch !== 'main') {
		refuse_or_warn(`${branch ? `on branch "${branch}"` : 'HEAD is detached'} — publish from main`);
	} else print('  Branch: main');
	const head = await git_rev(git, 'HEAD');
	if (head === null) refuse_or_warn('git rev-parse HEAD failed');

	// Origin sync — the published source must be the public source, and the final `git push`
	// runs after npm publish, when a diverged main is no longer recoverable by re-running.
	// A resume may sit one commit ahead, when that commit is its own release commit and only
	// the push failed.
	const compared = await git_fetch_and_compare_origin_main(git);
	if (!compared.ok) refuse_or_warn(`${compared.error} — cannot verify sync with origin`);
	else {
		const { ahead, behind } = compared;
		const own_release_commit =
			plan.kind === 'resume' &&
			behind === 0 &&
			ahead === 1 &&
			(await git_head_subject(git)) === release_commit_message(plan.version);
		if ((behind > 0 || ahead > 0) && !own_release_commit) {
			refuse_or_warn(
				`HEAD is ${ahead} ahead / ${behind} behind origin/main — release from exactly origin/main`
			);
		} else print('  Origin: in sync with origin/main');
	}

	// Git cleanliness. A retry legitimately carries the bump's own edits
	// (`RELEASE_PATHS`) but nothing else: a source fix made between attempts would
	// publish without landing in the release commit — and since the pushed tag
	// builds the native set, the wasm and napi artifacts would be built from
	// different source. Commit the fix, then re-run `--wetrun` to resume.
	const status = await git_status_lines(git);
	if (!status.ok) refuse_or_warn(status.error);
	else {
		const stray =
			plan.kind === 'fresh'
				? status.lines
				: status.lines.filter((line) => !RELEASE_PATHS.includes(git_status_line_path(line)));
		if (stray.length > 0) {
			const scope =
				plan.kind === 'fresh' ? '' : ` outside the release files (${RELEASE_PATHS.join(', ')})`;
			refuse_or_warn(`uncommitted changes${scope}:\n    ${stray.join('\n    ')}`);
		}
	}

	// The tag Step 8 pushes triggers release.yml, so it must land on the release commit:
	// a fresh release's tag must not exist yet (a stale local one would be pushed in its
	// place, building the native set from the wrong commit; one on origin would fail the push
	// after npm publish), and a resume's may exist only on HEAD.
	if (release_tag !== null) {
		const local = await git_local_tag_commit(git, release_tag);
		const remote = await git_remote_tag_commit(git, release_tag);
		if (!remote.ok) refuse_or_warn(remote.error);
		for (const [where, commit] of [
			['locally', local],
			['on origin', remote.ok ? remote.commit : null]
		] as const) {
			if (commit === null) continue;
			if (plan.kind === 'fresh') refuse_or_warn(`tag ${release_tag} already exists ${where}`);
			else if (commit !== head) {
				refuse_or_warn(`tag ${release_tag} exists ${where} on ${commit}, not on HEAD`);
			}
		}
	}

	const who = await npm_whoami(npm);
	if (who.ok) print(`  npm authenticated as: ${who.user}`);
	else refuse_or_warn(`not logged in to npm (run \`npm login\` first): ${who.error}`);
	// npm's dry-run setting turns a wetrun's `npm publish` into a dry run that exits 0, so the
	// release would go on to tag a version it never published; Step 7's confirmation catches any
	// such no-op, and this names the known cause before anything is bumped
	const dry_run_setting = await npm_dry_run_setting(npm);
	if (!dry_run_setting.ok) refuse_or_warn(dry_run_setting.error);
	else if (dry_run_setting.value !== 'false') {
		refuse_or_warn(
			`npm's dry-run setting is ${JSON.stringify(dry_run_setting.value)} (\`npm config get dry-run\`), ` +
				'so each `npm publish` would only pretend — unset `npm_config_dry_run` (npm reads it in any case, with `-` or `_`; `npm run … --dry-run` sets it) or remove `dry-run` from the npmrc that sets it'
		);
	}

	// node runs Step 6's artifact tests, which `node --test` executes as .ts by native type
	// stripping — so a node older than that fails there, after the bump, as plainly as a
	// missing one
	const node_version = await probe_version(programs.node, 'node');
	if (node_version === null) {
		print_error('  FAIL: node not found — required to test the built packages');
		exit(1);
	}
	const node_semver = /^v(\d+)\.(\d+)\./.exec(node_version);
	if (
		!node_semver ||
		Number(node_semver[1]) < NODE_MIN[0] ||
		(Number(node_semver[1]) === NODE_MIN[0] && Number(node_semver[2]) < NODE_MIN[1]) ||
		// odd 23.x took default type stripping only at 23.6
		(Number(node_semver[1]) === 23 && Number(node_semver[2]) < 6)
	) {
		print_error(
			`  FAIL: node ${node_version} — Step 6 needs >= ${NODE_MIN.join('.')} ` +
				'(node --test runs the .ts test files by native type stripping)'
		);
		exit(1);
	}
	print(`  node: ${node_version}`);

	// wasm-pack builds every WASM bundle in Step 4 — asked here so a missing one fails
	// before the bump rather than after it
	const wasm_pack_version = await probe_version(programs.wasm_pack, 'wasm-pack');
	if (wasm_pack_version === null) {
		print_error(
			'  FAIL: wasm-pack not found — required to build the packages (`cargo install wasm-pack`)'
		);
		exit(1);
	}
	print(`  ${wasm_pack_version}`);

	// Step 6 runs the parse-failure table under Bun (`deno task test:bun`), the one leg that
	// grades the engines' returned failures under Bun — asked here so a
	// --wetrun without bun fails before the bump. Waived like Step 3b's oracles: a dry run or
	// --no-check warn-skips, re-warned in the final summary.
	let bun_skip_reason: string | null = null;
	const bun_version = await probe_version(programs.bun, 'bun');
	if (bun_version !== null) {
		print(`  bun: ${bun_version}`);
	} else if (wetrun && !no_check) {
		print_error(
			'  FAIL: bun not found — required to run the parse-failure table under Bun (deno task test:bun). ' +
				'Install bun, or pass --no-check to release without it (explicitly).'
		);
		exit(1);
	} else {
		bun_skip_reason = no_check ? 'bun not found; waived by --no-check' : 'bun not found';
		print_error(
			`  WARN: ${bun_skip_reason} — Step 6 skips the Bun leg` +
				(wetrun || no_check ? '' : '; a --wetrun would FAIL here')
		);
	}

	// Step 6's declaration check loads TypeScript and @types/node from the benches/js
	// install — asked here so a missing install fails before the bump and the build
	const missing_typecheck_install: Array<string> = [];
	for (const { what, path } of TYPECHECK_INSTALL) {
		if (!(await exists(path))) missing_typecheck_install.push(what);
	}
	if (missing_typecheck_install.length > 0) {
		print_error(
			`  FAIL: benches/js/node_modules lacks ${missing_typecheck_install.join(' + ')} — ` +
				'required to type-check the built packages (deno task typecheck:packages); ' +
				'run `deno task bench:install`'
		);
		exit(1);
	}

	// Step 3's `deno task check` and Step 3b's gates grade the fixtures through tsv_debug's sidecar,
	// which refuses to run without its packages installed as its lock says — asked here so a
	// missing or stale install fails before the bump rather than halfway through Step 3
	if (!no_check) {
		const sidecar = sidecar_install_state(checkout.read_text_sync(SIDECAR_LOCK_PATH), (path) =>
			checkout.exists_sync(`${SIDECAR_DIR}/${path}`)
				? checkout.read_text_sync(`${SIDECAR_DIR}/${path}`)
				: null
		);
		if (sidecar !== 'current') {
			print_error(
				`  FAIL: the sidecar's packages ${sidecar === 'missing' ? 'are not installed' : 'differ from its lock'} — ` +
					`required by Step 3's fixture oracle; run \`${SIDECAR_INSTALL_COMMAND}\``
			);
			exit(1);
		}
		print('  sidecar: installed as its lock says');
	}

	// The N-API half of the release is a different machine's: the tag Step 8 pushes triggers
	// release.yml, AFTER this script has published the wasm packages — so a matrix that
	// fails then leaves a half-released version. Its dispatch dry run on this commit is the
	// rehearsal that answers that ahead of time, and nothing here can ask GitHub for it.
	print(
		'  REMINDER: the release.yml workflow_dispatch dry run must be green on this commit ' +
			'before a --wetrun (the wasm packages publish before the tag starts the N-API matrix)'
	);

	// Registry. A fresh release's version must be unpublished everywhere; a resume skips what
	// is already out, and one whose packages are all out only has git left to finish.
	let published_count = 0;
	if (release_version !== null) {
		for (const { label } of packages) {
			const published = await npm_is_published(npm, label, release_version);
			if (!published.ok) failures.push(published.error);
			else if (published.published) {
				if (plan.kind === 'fresh') failures.push(`${label}@${release_version} is already on npm`);
				else published_count++;
			}
		}
	}
	const all_published = published_count === packages.length;

	if (failures.length > 0 || refusals.length > 0) {
		for (const message of [...failures, ...refusals]) print_error(`  FAIL: ${message}`);
		print_error(
			`  Refusing to ${wetrun ? 'publish' : 'run'} — nothing was changed, built, or published.`
		);
		exit(1);
	}

	// Step 2: Bump

	/** The version the build stamps: the bumped one under --wetrun, the checkout's in a dry run. */
	let version: string = version_before;

	if (wetrun) {
		print('\n=== Step 2: Bump ===');
		if (release_version === null) {
			print_error('  FAIL: no release version — the plan should have refused');
			exit(1);
		}
		if (plan.kind === 'fresh') {
			if (plan.stale_sentinel !== null) {
				print_error(`  WARN: removing stale sentinel (v${plan.stale_sentinel})`);
				await sentinel_remove(checkout);
			}
			const written = await bump_cargo_version(checkout, release_version);
			if (!written.ok) {
				print_error(`  FAIL: ${written.error}`);
				exit(1);
			}
			// the sentinel right after the version write: everything below is idempotent and
			// re-runs on a resume, so any later failure is resumable
			await sentinel_write(checkout, release_version);
			print(`  Version bumped (${plan.level}): ${plan.version} -> ${release_version}`);
		} else {
			print(`  Resuming v${release_version} — bump already applied`);
		}
		bumped = true;
		version = release_version;
		const locked = await bump_lockfile(programs.cargo, checkout);
		if (!locked.ok) fail_resumable(locked.error);
		const stamped = await bump_changelog(checkout, release_version);
		if (stamped === 'no_unreleased') {
			fail_resumable(`no "## Unreleased" or "## ${release_version}" in ${CHANGELOG_PATH}`);
		}
		print(
			stamped === 'already_stamped'
				? `  ${CHANGELOG_PATH} already stamped with ## ${release_version}`
				: `  Stamped ${CHANGELOG_PATH}: ## Unreleased -> ## ${release_version}; seeded fresh ## Unreleased (bump: patch)`
		);
	} else {
		print('\n=== Step 2: Bump (dry-run — nothing written) ===');
		if (plan.kind === 'resume') print(`  A wetrun resumes v${plan.version}`);
		else if (plan.target !== null) print(`  A wetrun bumps to v${plan.target}`);
	}

	/** Step 8: commit exactly the release files, tag the commit, push both, then remove the
	 * sentinel — or, under `--no-git`, print those steps. Past Step 7 the packages are public,
	 * so a failure here is the one place a bare exit code is not enough: it names the exact
	 * recovery. Every step no-ops when already done, so a resume repeats it safely. */
	const finalize_git = async (): Promise<void> => {
		// every dry-run path exits before this; it guards a future edit that falls through
		if (!wetrun) {
			print_error('  FAIL: a dry run reached the git finalize — refusing to commit, tag, or push');
			exit(1);
		}
		const tag = version_tag(version);
		const message = release_commit_message(version);
		const manual_finalize = [
			`    git add ${RELEASE_PATHS.join(' ')}`,
			`    git commit -m "${message}"`,
			`    git tag -m ${tag} ${tag}`,
			'    git push --follow-tags origin main'
		].join('\n');
		const git_fail: (error: string) => never = (error) => {
			print_error(`\n  FAIL: ${error}`);
			print_error(
				'  The packages are PUBLISHED — only the release commit + tag + push is missing.\n' +
					'  The sentinel survives this step, so `deno task publish --wetrun` resumes from here\n' +
					'  (it skips straight to this step once every package is on npm). Or finalize by hand:\n' +
					manual_finalize
			);
			return exit(1);
		};

		if (no_git) {
			print('\n=== Step 8: Git finalize — SKIPPED (--no-git) ===');
			print('  Finalize manually (only the release files; annotated tag — --follow-tags');
			print('  ignores lightweight tags):');
			print(manual_finalize);
		} else {
			print('\n=== Step 8: Git finalize (commit + tag + push) ===');
			const committed = await git_commit_paths(git, RELEASE_PATHS, message);
			if (!committed.ok) git_fail(committed.error);
			print(committed.committed ? `  Committed "${message}"` : '  Release commit already made');
			if ((await git_local_tag_commit(git, tag)) === null) {
				const tagged = await git_tag_annotated(git, tag);
				if (!tagged.ok) git_fail(tagged.error);
				print(`  Tagged ${tag}`);
			} else print(`  Tag ${tag} already exists`);
			// this push is what triggers release.yml — until it lands, the wasm packages
			// are public with no tag, so the N-API set never builds
			if (!(await git_push_main_with_tags(git, print_error))) {
				git_fail('git push --follow-tags origin main failed');
			}
		}

		// last: the release is complete only once the commit, tag and push have landed (or
		// --no-git handed them off); removing it earlier would make a finalize failure
		// unresumable, since the fresh path refuses the `## Unreleased` this run already stamped
		await sentinel_remove(checkout);
	};

	if (all_published) {
		print(`\n  Every package is already on npm at v${version} — only git remains.`);
		if (!wetrun) {
			print('  Dry-run complete — `deno task publish --wetrun` finishes the release.\n');
			return;
		}
		await finalize_git();
		print('');
		return;
	}

	/** The checkout as the build reads it; Step 6 compares against it before anything publishes. */
	const checkout_before = await git_checkout_fingerprint(git);

	// Step 3: Check

	if (no_check) {
		print('\n=== Step 3: Check — SKIPPED (--no-check) ===');
	} else {
		print('\n=== Step 3: Check (deno task check) ===');
		await run('deno task check', programs.deno, 'deno', ['task', 'check']);
		// The depth-limit stack contract in the shipped binaries' optimization (the `napi`
		// profile, without the profile guidance the published pair adds): `check`'s
		// `test:depth` grades the `corpus` world, whose frames are further still from what
		// ships. The profile-guided addon itself is graded by the release workflow on every
		// row (docs/tasks.md §`test:depth:napi`).
		print('\n=== Step 3: Depth contract, napi profile (deno task test:depth:napi) ===');
		await run('deno task test:depth:napi', programs.deno, 'deno', ['task', 'test:depth:napi']);
	}

	// Step 3b: Conformance gates
	//
	// The release-cadence correctness gates that run against EXTERNAL oracles and so
	// can't live in `deno task check`: the Svelte/TS parsers vs their own suites +
	// svelte/compiler / acorn-typescript / tsc baselines (conformance:svelte-fixtures
	// / ts-fixtures / ts-repo), all three languages' AST + formatter vs the canonical
	// parsers / prettier (corpus:compare:parse + :format, --all), and the JS parser vs
	// test262 POSITIVES (conformance:test262, pure Rust — negatives are the deferred
	// early-error frontier, reported not gated). `deno task conformance:all` runs the
	// whole set behind its preflights (`pins:audit:checkouts`, `bench:pins:suites` —
	// which re-derives the suite-derived count pins nothing else grades, so a corpus
	// that moved under them fails the release rather than shipping numbers describing
	// the previous one — and the two oracle-freshness fixture legs). Running them here
	// means a release can't ship a parse/format regression the in-repo gate is
	// structurally blind to. Skipped by --no-check alongside Step 3.
	//
	// The gates need the ../svelte + ../acorn-typescript + ../typescript +
	// ../test262 checkouts, the corpus legs' pinned inputs (../corpora — the real-code
	// snapshot — plus ../prettier and ../prettier-plugin-svelte; CorpusLoader fails
	// fast on a missing non-optional entry), and the benches/js `node_modules` sidecar
	// (`deno task bench:install`). The probe must cover every oracle the aggregate's legs need
	// — the gates themselves fail closed on a missing checkout, so the probe is the
	// ONE tolerance point: a dry-run warn-and-skips (clean machines can still
	// validate), but a --wetrun BLOCKS — releasing with the gates never run
	// requires the explicit --no-check. Any skip is re-warned in the final summary
	// so it can't scroll away. Only the CSS-WPT harvest stays manual.
	let conformance_skip_reason: string | null = null;
	if (no_check) {
		print('\n=== Step 3b: Conformance gates — SKIPPED (--no-check) ===');
		conformance_skip_reason = '--no-check';
	} else {
		print('\n=== Step 3b: Conformance gates (deno task conformance:all) ===');
		const missing: Array<string> = [];
		for (const [path, what] of [
			['../svelte/packages/svelte/tests', '../svelte checkout'],
			['../acorn-typescript/test', '../acorn-typescript checkout'],
			['../typescript/tests', '../typescript checkout'],
			['../test262/test', '../test262 checkout'],
			['../prettier/tests/format', '../prettier checkout'],
			['../prettier-plugin-svelte/test', '../prettier-plugin-svelte checkout'],
			[deps.corpora.collections, `${deps.corpora.root} checkout (the real-code snapshot)`],
			['benches/js/node_modules', 'benches/js/node_modules (deno task bench:install)']
		] as const) {
			if (!(await exists(path))) missing.push(what);
		}
		if (missing.length > 0 && wetrun) {
			print_error(
				`  FAIL: missing ${missing.join(' + ')} — the conformance gates cannot run. ` +
					'Clone/install the missing pieces, or pass --no-check to release without gates (explicitly).'
			);
			exit(1);
		} else if (missing.length > 0) {
			conformance_skip_reason = `missing ${missing.join(' + ')}`;
			print_error(
				`  WARN: skipping — ${conformance_skip_reason}. ` +
					'A --wetrun would FAIL here; run `deno task conformance:all` on a machine with the oracles.'
			);
		} else {
			await run('deno task conformance:all', programs.deno, 'deno', ['task', 'conformance:all'], {
				fail_hint:
					'  Conformance gate failed. A corpus:compare:format SAFETY hit is self-verified in-run\n' +
					'  (the native format re-runs and must reproduce byte-identically), so treat it as a real\n' +
					'  regression; FFI corruption surfaces instead as a per-file `native format\n' +
					'  nondeterminism` error (benches/js/CLAUDE.md §Known Issues). A conformance:test262\n' +
					'  GATE FAIL is a real positive-parse regression (or a deliberate test262 pull needing a\n' +
					'  POSITIVE_PASSED_PIN re-pin).'
			});
		}
	}

	// Step 3c: Corpus robustness audit
	//
	// The pure-Rust content-loss / panic / non-idempotency audits over REAL code (the robustness
	// corpus view — the WHOLE ../corpora snapshot, every collection it vendors, plus the
	// svelte_styles cache and the live diff of this machine's working trees against the snapshot
	// — and the pinned Prettier suites). This is the
	// extension-robustness bar: `tests/fixtures` is format-stable by construction, so `deno task check`
	// is structurally blind to a content-loss / panic / reflow bug in real code — such bugs are found
	// by a corpus audit or a wide fuzz seed, never by `check`. `deno task audit:corpus`
	// warn-skips absent working trees, so its reproducible floor is the whole ../corpora snapshot;
	// like Step 3b, a --wetrun without that floor BLOCKS (releasing without the audit needs --no-check),
	// a dry-run warn-and-skips. The FFI/prettier SAFETY half (content loss vs prettier) rides Step 3b's
	// corpus:compare:format --all; this is the pure-Rust half those gates can't reach.
	let audit_corpus_skip_reason: string | null = null;
	if (no_check) {
		print('\n=== Step 3c: Corpus robustness audit — SKIPPED (--no-check) ===');
		audit_corpus_skip_reason = '--no-check';
	} else {
		print('\n=== Step 3c: Corpus robustness audit (deno task audit:corpus) ===');
		if (!(await exists(deps.corpora.collections))) {
			audit_corpus_skip_reason = `missing ${deps.corpora.root} checkout (the real-code snapshot — the corpus floor)`;
			if (wetrun) {
				print_error(
					`  FAIL: ${audit_corpus_skip_reason} — the corpus audit cannot run over real code. ` +
						'Clone it, or pass --no-check to release without the audit (explicitly).'
				);
				exit(1);
			}
			print_error(
				`  WARN: skipping — ${audit_corpus_skip_reason}. ` +
					'A --wetrun would FAIL here; run `deno task audit:corpus` on a machine with the corpus.'
			);
		} else {
			await run('deno task audit:corpus', programs.deno, 'deno', ['task', 'audit:corpus'], {
				fail_hint:
					'  Corpus robustness audit found a content-loss / panic / non-idempotency regression over\n' +
					'  REAL code (what `deno task check`’s fixture-only gate cannot see). Triage the failing\n' +
					'  leg by name with the matching `tsv_debug` audit on the single repo it points at.'
			});
		}
	}

	// Step 4: Build

	print('\n=== Step 4: Build WASM bundles (npm + deno) ===');
	// Single source of truth for the publishable artifact build: the `build:packages`
	// task in deno.json. It builds everything Step 6 validates, including the
	// unpublished deno bundles (stale bundles must never gate or falsely pass a
	// publish), and is ordered to group cargo feature sets (format, format, parse,
	// parse, default, default) so the wasm crate compiles once per feature set, not
	// once per task. CI's artifacts job runs the same task, so the two can't drift.
	// A release build must never freshness-skip: the wasm build tasks ride
	// scripts/run_if_stale.ts, whose mtime check can't see toolchain changes
	// (wasm-pack/wasm-opt/rustc upgrades). `programs.deno_build` carries
	// `TSV_BUILD_FORCE=1`, forcing the whole build step.
	await run('deno task build:packages', programs.deno_build, 'deno', ['task', 'build:packages']);

	// Step 5: Verify built packages

	print('\n=== Step 5: Verify built package name + version ===');
	for (const { label, dir } of packages) {
		const built_pkg = JSON.parse(await checkout.read_text(`${dir}/package.json`));
		if (built_pkg.name !== label) {
			fail_resumable(`${dir} built as ${built_pkg.name}, expected ${label}`);
		}
		if (built_pkg.version !== version) {
			fail_resumable(`${label} built at v${built_pkg.version}, expected v${version}`);
		}
		// `npm publish` runs in the staged directory, beyond the root `.npmrc`'s `ignore-scripts`,
		// so a staged manifest's own `prepublishOnly`, `prepack`, `prepare`, `postpack`, `publish`,
		// or `postpublish` would run with the owner's credentials
		if (built_pkg.scripts !== undefined) {
			const keys =
				typeof built_pkg.scripts === 'object' && built_pkg.scripts !== null
					? Object.keys(built_pkg.scripts)
					: [];
			fail_resumable(
				`${label} (${dir}/package.json) has a \`scripts\` key${keys.length > 0 ? ` (${keys.join(', ')})` : ''}, ` +
					"which `npm publish` would run with the owner's credentials — a staged package carries none"
			);
		}
		print(`  PASS: ${label} = v${version}`);
	}

	// Step 6: Test the built artifacts

	print('\n=== Step 6: Validate built packages (sizes + Node + Bun + declarations) ===');
	await run('deno task validate:artifacts', programs.deno, 'deno', ['task', 'validate:artifacts']);
	// A second spelling of the `test:npm*:run` tasks (see deno.json's `//test:npm:run`): a leg
	// added there must be added here too, or a publish silently skips it.
	for (const { label, dir } of packages) {
		await run(`artifact tests for ${label}`, programs.node_test(dir), 'node', [
			'--test',
			'scripts/test_npm.ts'
		]);
	}
	// The depth limits' stack contract in the WASM engine, over the `all` package just built:
	// every shape the native gate generates, at its accepted depth, on a Node main thread, cold
	// and tiered up (docs/tasks.md §`test:depth:wasm`). A `check`-cadence gate like Step 3's
	// shipped-build leg, so `--no-check` skips it with that one.
	if (no_check) {
		print_error('  WARN: skipping deno task test:depth:wasm (--no-check)');
	} else {
		await run('deno task test:depth:wasm', programs.deno, 'deno', ['task', 'test:depth:wasm']);
	}
	// The parse-failure table under Bun, the one runtime that grades the engines' point
	// properties (docs/tasks.md §`test:bun`), over the three wasm packages this script publishes —
	// the napi engine under Bun is graded locally by `test:napi:npm`, not here. Bun's presence
	// was settled by the preflight, so the leg requires it.
	if (bun_skip_reason === null) {
		await run('deno task test:bun', programs.deno, 'deno', [
			'task',
			'test:bun',
			'--require-bun',
			'format',
			'parse',
			'all'
		]);
	} else {
		print_error(`  WARN: skipping deno task test:bun — ${bun_skip_reason}`);
	}
	// The merged published declarations, as a consumer's compiler sees them, for every npm
	// package this release ships. The napi loader publishes from release.yml, but its
	// staging is a file copy (no cargo build), so it is staged here at this version and graded
	// beside the three wasm packages — all four named, so each is required. Needs the
	// benches/js install whatever --no-check says: it grades what is about to ship.
	await run('stage the @fuzdev/tsv loader', programs.node, 'node', [
		'scripts/roots/build_napi_packages.ts',
		'--loader-only'
	]);
	await run('deno task typecheck:packages', programs.deno, 'deno', [
		'task',
		'typecheck:packages',
		'format',
		'parse',
		'all',
		'napi'
	]);

	// The packages hold what the checkout held while Step 4 built them: a checkout that moved
	// since the bump refuses a wetrun rather than publish packages no commit describes.
	const checkout_after = await git_checkout_fingerprint(git);
	if (
		!checkout_before.ok ||
		!checkout_after.ok ||
		checkout_before.fingerprint !== checkout_after.fingerprint
	) {
		const moved =
			'the checkout changed while the gates ran — the built packages may match no commit';
		if (wetrun) fail_resumable(`${moved}; refusing to publish`);
		warnings++;
		print_error(`  WARN: ${moved}`);
	}

	// Step 7: Publish

	print('\n=== Step 7: Publish to npm ===');

	// Publish loop is idempotent: already-published packages are skipped, so retry after
	// partial publish works automatically.

	/** Count `label` as published only once the registry lists it at `version`: an `npm
	 * publish` that exits 0 may have published nothing (`PUBLISH_CONFIRM_READS`), and git must
	 * never tag a version that isn't out. Gives up resumable, before any git call. */
	const confirm_published = async (label: string): Promise<void> => {
		const wait_s = PUBLISH_CONFIRM_RETRY_MS / 1000;
		let last = '';
		for (let read = 1; read <= PUBLISH_CONFIRM_READS; read++) {
			const listed = await npm_is_published(npm, label, version);
			if (listed.ok && listed.published) return;
			last = listed.ok ? '' : `\n${listed.error}`;
			if (read === PUBLISH_CONFIRM_READS) break;
			print(
				`  ${listed.ok ? `${label}@${version} is not on the registry yet` : `npm view ${label}@${version} failed`}` +
					` — reading again in ${wait_s}s (${read}/${PUBLISH_CONFIRM_READS})`
			);
			await deps.sleep(PUBLISH_CONFIRM_RETRY_MS);
		}
		const waited_min = ((PUBLISH_CONFIRM_READS - 1) * PUBLISH_CONFIRM_RETRY_MS) / 60_000;
		print_error(
			`  FAIL: npm publish ${label} exited 0, but the registry does not list ${label}@${version} ` +
				`after ${waited_min} minutes${last}`
		);
		print_error(
			'  Either the publish did not happen (npm ran it as a dry run) or the registry is behind.\n' +
				'  Nothing was committed, tagged, or pushed. A resume skips it once the registry lists it\n' +
				'  and publishes it while it is absent (a registry still behind then refuses that second\n' +
				'  publish, and the run stops here again).'
		);
		exit_resumable();
	};

	for (let i = 0; i < packages.length; i++) {
		const { label, dir } = packages[i]!;
		if (wetrun) {
			const published = await npm_is_published(npm, label, version);
			if (!published.ok) fail_resumable(published.error);
			if (published.published) {
				print(`  SKIP: ${label}@${version} already published`);
				continue;
			}
			print(`  Publishing ${label}...`);
			const not_published = packages.slice(i);
			const fail_hint = `  Packages not yet published (a resume retries them, or publish by hand):\n${not_published
				.map((p) => `    cd ${p.dir} && npm publish --access public`)
				.join('\n')}`;
			await run(`npm publish ${label}`, programs.npm, 'npm', ['publish', '--access', 'public'], {
				cwd: checkout.dir(dir),
				fail_hint
			});
			await confirm_published(label);
			print(`  Published ${label}@${version}`);
		} else {
			print(`  [dry-run] ${label}:`);
			const res = await program_capture(
				programs.npm,
				'npm',
				['publish', '--dry-run', '--access', 'public'],
				{ cwd: checkout.dir(dir), stdin: 'inherit' }
			);
			// npm prints the tarball notice to stderr — surface it regardless.
			if (res.err) print(res.err);
			if (res.success) {
				print(`  PASS: ${label} packs cleanly`);
			} else if (plan.kind === 'fresh' && npm_error_is_republish(res.err)) {
				// Dry-run never bumps, so it dry-publishes the CURRENT (already-published)
				// version. The real wetrun bumps first, so this isn't a real failure.
				const target = plan.target === null ? 'the bumped version' : `v${plan.target}`;
				print(
					`  PASS: ${label} packs cleanly (v${version} already published — a real run publishes ${target})`
				);
			} else {
				if (res.out) print_error(res.out);
				print_error(`  FAIL: npm publish --dry-run ${label}`);
				exit(1);
			}
		}
	}

	// Step 8: Git finalize

	if (wetrun) await finalize_git();

	// Summary

	print('\n=== Done ===');
	for (const { label, dir } of packages) {
		const wasm_path = `${dir}/tsv_wasm_bg.wasm`;
		const info = await checkout.stat(wasm_path);
		if (info === null) throw new Error(`${checkout.path}/${wasm_path} does not exist`);
		if (programs.gzip === null) throw new Error('gzip is not on PATH');
		const gzipped = await gzip_size(programs.gzip, `${checkout.path}/${wasm_path}`);
		const size_note = `wasm ${format_size(info.size)} (${format_size(gzipped)} gzipped wire)`;
		print(
			wetrun
				? `  Published ${label}@${version} — ${size_note}`
				: `  ${label}@${version} — ${size_note}`
		);
	}
	if (wetrun) {
		// This script releases the WASM half only. The N-API set publishes from
		// release.yml, triggered by the tag Step 8 pushes — so a green run here
		// is not a finished release, and saying so beats leaving it to the runbook.
		// (Before the skip warnings below: those keep the last word.)
		print(
			no_git
				? `\n  The N-API set does NOT publish until the v${version} tag is pushed —`
				: `\n  The pushed v${version} tag triggers release.yml —`
		);
		print('  the N-API set (@fuzdev/tsv + its platform packages) publishes there.');
		print(
			no_git
				? '  Finish the finalize above, then watch that run.'
				: '  The release is complete only once that run is green; if the matrix fails, re-run it\n' +
						'  from the Actions tab — dispatching ON THE TAG, so the version assertion still holds.'
		);
		// The crates.io half is separate again (scripts/publish_crates.ts): that workflow's
		// `crates` job only rehearses it, and the crates publish last, after the GitHub Release.
		print(
			'  The crates publish last, once that run has made the GitHub Release:\n' +
				`  \`deno task publish:crates --wetrun\` from a clean checkout of v${version}, which it\n` +
				'  requires (its `crates` job is a dry run).'
		);
	}
	if (conformance_skip_reason !== null) {
		// Re-warn at the very end — the Step 3b warning is a single line buried in
		// build output, and a release whose conformance gates never ran must be the
		// last thing on screen, not a scrolled-away footnote.
		print_error(`\n  ⚠ Step 3b conformance gates did NOT run (${conformance_skip_reason}).`);
		print_error('    Run `deno task conformance:all` and verify it passes for this release.');
	}
	if (audit_corpus_skip_reason !== null) {
		// Re-warn at the very end — same reasoning as the Step 3b re-warn: a release whose real-code
		// robustness audit never ran must be the last thing on screen, not a scrolled-away footnote.
		print_error(`\n  ⚠ Step 3c corpus robustness audit did NOT run (${audit_corpus_skip_reason}).`);
		print_error('    Run `deno task audit:corpus` and verify it passes for this release.');
	}
	if (bun_skip_reason !== null) {
		// Re-warn at the very end, beside the Step 3b/3c re-warns: the engines' Bun-only point
		// properties went ungraded for this release.
		print_error(`\n  ⚠ Step 6 parse-failure table under Bun did NOT run (${bun_skip_reason}).`);
		print_error(
			'    Run `deno task test:bun format parse all` with bun installed for this release.'
		);
	}
	if (!wetrun) {
		if (warnings > 0) {
			print(`\n  Dry-run complete for v${version} — a wetrun would FAIL on the warnings above.`);
		} else if (plan.kind === 'resume') {
			print(`\n  Dry-run complete for v${version} — all checks passed.`);
			print('  Run `deno task publish --wetrun` to resume the release.');
		} else {
			print(`\n  Dry-run complete for v${version} — all checks passed.`);
			print(`  Run with --wetrun --bump ${plan.level} to publish v${plan.target}.`);
		}
	}
	print('');
}

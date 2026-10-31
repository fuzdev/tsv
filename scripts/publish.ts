/**
 * Publish script for the tsv npm packages.
 *
 * Dry-run by default — runs all validation but does not mutate the workspace.
 * Pass `--wetrun` to bump, build, validate, publish to npm, and finalize git
 * (commit + tag + push). The build step covers the npm packages AND the deno
 * bundles — the deno bundles aren't published, but artifact validation checks
 * every built bundle, so they're rebuilt here to never gate on stale artifacts.
 *
 * Version source of truth: `Cargo.toml [workspace.package] version` (read
 * directly by wasm-pack). There is no root package.json and no changesets —
 * `--bump` edits Cargo.toml and converts CHANGELOG.md's `## Unreleased`
 * section into the new version's section (the grammar is `scripts/release/changelog.ts`;
 * that stamped section is what the tag's GitHub Release reads back as its body,
 * via `scripts/release_notes.ts` in the release workflow). What a run releases
 * — fresh or resumed, and every reason a fresh one cannot — is decided once by
 * `scripts/release/plan.ts`; the dry run warns on exactly what a wetrun refuses.
 *
 * Usage:
 *   deno task publish --bump patch           # dry-run (validate everything, preview the bump)
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
 * and a failing `npm whoami` — refuse a wetrun and warn a dry run; a missing tool, a
 * failed registry lookup, and a fresh release whose version is already on npm refuse
 * both.
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
 */

import { CORPORA_COLLECTIONS, CORPORA_ROOT } from '../benches/js/lib/corpora.ts';
import {
	bump_cargo_version,
	bump_changelog,
	bump_lockfile,
	release_commit_message,
	RELEASE_PATHS
} from './release/bump.ts';
import { CHANGELOG_PATH } from './release/changelog.ts';
import { command_capture, type CommandOptions, command_stream } from './release/command.ts';
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
	git_tag_annotated
} from './release/git.ts';
import { npm_error_is_republish, npm_is_published, npm_whoami } from './release/npm.ts';
import { release_plan } from './release/plan.ts';
import { sentinel_read, sentinel_remove, sentinel_write } from './release/sentinel.ts';
import {
	type BumpLevel,
	CARGO_PATH,
	cargo_workspace_version,
	is_bump_level,
	version_tag
} from './release/version.ts';
import { format_size, gzip_size } from './size.ts';
import { TYPECHECK_INSTALL } from './typecheck_packages.ts';

const USAGE =
	'Usage: deno task publish [--wetrun] [--bump patch|minor|major] [--no-check] [--no-git]';

// A misspelled --wetrun fails safe (dry-run), but a misspelled --no-git or
// --no-check fails open — reject anything unrecognized.
let wetrun = false;
let no_check = false;
let no_git = false;
let bump: BumpLevel | null = null;
const unknown_args: Array<string> = [];
for (let i = 0; i < Deno.args.length; i++) {
	const arg = Deno.args[i];
	if (arg === '--wetrun') wetrun = true;
	else if (arg === '--no-check') no_check = true;
	else if (arg === '--no-git') no_git = true;
	else if (arg === '--bump') {
		const level = Deno.args[++i];
		if (!is_bump_level(level)) {
			console.error(
				`FAIL: --bump must be patch, minor, or major (got ${JSON.stringify(level ?? '')})`
			);
			Deno.exit(1);
		}
		bump = level;
	} else unknown_args.push(arg);
}
if (unknown_args.length > 0) {
	console.error(`FAIL: unknown argument(s): ${unknown_args.join(' ')}`);
	console.error(USAGE);
	Deno.exit(1);
}

/** The oldest node Step 6 runs on — the first with native TypeScript type stripping
 * unflagged, which `node --test` needs to execute the `.ts` test files directly. */
const NODE_MIN = [22, 18] as const;

/** The published packages, in publish order. */
const packages = [
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

console.log(`\n=== tsv publish ${wetrun ? '(wetrun)' : '(dry-run)'} ===\n`);

// Step 1: Preflight checks

console.log('=== Step 1: Preflight checks ===');

const version_before = cargo_workspace_version(Deno.readTextFileSync(CARGO_PATH));
if (version_before === null) {
	console.error('  FAIL: could not find [workspace.package] version in Cargo.toml');
	Deno.exit(1);
}
const plan = release_plan({
	version: version_before,
	bump,
	sentinel: await sentinel_read(),
	changelog: read_changelog()
});
/** The version this run releases; null only when a fresh plan has no target, which a
 * wetrun refuses. */
const release_version = plan.kind === 'resume' ? plan.version : plan.target;
const release_tag = release_version === null ? null : version_tag(release_version);

if (plan.kind === 'resume') {
	console.log(`  Sentinel found at v${plan.version} — resuming that release`);
	for (const note of plan.notes) console.warn(`  WARN: ${note}`);
} else {
	console.log(
		`  Version: ${plan.version} -> ${plan.target ?? '?'}${plan.level ? ` (${plan.level})` : ''}`
	);
	if (plan.stale_sentinel !== null) {
		console.warn(`  WARN: stale sentinel (v${plan.stale_sentinel}) — a fresh wetrun removes it`);
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
		console.warn([`  WARN: ${first} — a wetrun would FAIL`, ...rest].join('\n'));
	}
};

if (plan.kind === 'fresh') for (const problem of plan.problems) refuse_or_warn(problem);

const branch = await git_current_branch();
if (branch === null) refuse_or_warn('git branch failed — is this a git repository?');
else if (branch !== 'main') {
	refuse_or_warn(`${branch ? `on branch "${branch}"` : 'HEAD is detached'} — publish from main`);
} else console.log('  Branch: main');
const head = await git_rev('HEAD');
if (head === null) refuse_or_warn('git rev-parse HEAD failed');

// Origin sync — the published source must be the public source, and the final `git push`
// runs after npm publish, when a diverged main is no longer recoverable by re-running.
// A resume may sit one commit ahead, when that commit is its own release commit and only
// the push failed.
const compared = await git_fetch_and_compare_origin_main();
if (!compared.ok) refuse_or_warn(`${compared.error} — cannot verify sync with origin`);
else {
	const { ahead, behind } = compared;
	const own_release_commit =
		plan.kind === 'resume' &&
		behind === 0 &&
		ahead === 1 &&
		(await git_head_subject()) === release_commit_message(plan.version);
	if ((behind > 0 || ahead > 0) && !own_release_commit) {
		refuse_or_warn(
			`HEAD is ${ahead} ahead / ${behind} behind origin/main — release from exactly origin/main`
		);
	} else console.log('  Origin: in sync with origin/main');
}

// Git cleanliness. A retry legitimately carries the bump's own edits
// (`RELEASE_PATHS`) but nothing else: a source fix made between attempts would
// publish without landing in the release commit — and since the pushed tag
// builds the native set, the wasm and napi artifacts would be built from
// different source. Commit the fix, then re-run `--wetrun` to resume.
const status = await git_status_lines();
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
	const local = await git_local_tag_commit(release_tag);
	const remote = await git_remote_tag_commit(release_tag);
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

const who = await npm_whoami();
if (who.ok) console.log(`  npm authenticated as: ${who.user}`);
else refuse_or_warn(`not logged in to npm (run \`npm login\` first): ${who.error}`);

// node runs Step 6's artifact tests, which `node --test` executes as .ts by native type
// stripping — so a node older than that fails there, after the bump, as plainly as a
// missing one
const node_version = await probe_version('node');
if (node_version === null) {
	console.error('  FAIL: node not found — required to test the built packages');
	Deno.exit(1);
}
const node_semver = /^v(\d+)\.(\d+)\./.exec(node_version);
if (
	!node_semver ||
	Number(node_semver[1]) < NODE_MIN[0] ||
	(Number(node_semver[1]) === NODE_MIN[0] && Number(node_semver[2]) < NODE_MIN[1]) ||
	// odd 23.x took default type stripping only at 23.6
	(Number(node_semver[1]) === 23 && Number(node_semver[2]) < 6)
) {
	console.error(
		`  FAIL: node ${node_version} — Step 6 needs >= ${NODE_MIN.join('.')} ` +
			'(node --test runs the .ts test files by native type stripping)'
	);
	Deno.exit(1);
}
console.log(`  node: ${node_version}`);

// wasm-pack builds every WASM bundle in Step 4 — asked here so a missing one fails
// before the bump rather than after it
const wasm_pack_version = await probe_version('wasm-pack');
if (wasm_pack_version === null) {
	console.error(
		'  FAIL: wasm-pack not found — required to build the packages (`cargo install wasm-pack`)'
	);
	Deno.exit(1);
}
console.log(`  ${wasm_pack_version}`);

// Step 6 runs the parse-failure table under Bun (`deno task test:bun`), the one leg that
// grades the engines' returned failures under Bun — asked here so a
// --wetrun without bun fails before the bump. Waived like Step 3b's oracles: a dry run or
// --no-check warn-skips, re-warned in the final summary.
let bun_skip_reason: string | null = null;
const bun_version = await probe_version('bun');
if (bun_version !== null) {
	console.log(`  bun: ${bun_version}`);
} else if (wetrun && !no_check) {
	console.error(
		'  FAIL: bun not found — required to run the parse-failure table under Bun (deno task test:bun). ' +
			'Install bun, or pass --no-check to release without it (explicitly).'
	);
	Deno.exit(1);
} else {
	bun_skip_reason = no_check ? 'bun not found; waived by --no-check' : 'bun not found';
	console.warn(
		`  WARN: ${bun_skip_reason} — Step 6 skips the Bun leg` +
			(wetrun || no_check ? '' : '; a --wetrun would FAIL here')
	);
}

// Step 6's declaration check loads TypeScript and @types/node from the benches/js
// install — asked here so a missing install fails before the bump and the build
const missing_typecheck_install = TYPECHECK_INSTALL.filter(({ path }) => !exists(path));
if (missing_typecheck_install.length > 0) {
	console.error(
		`  FAIL: benches/js/node_modules lacks ${missing_typecheck_install.map((m) => m.what).join(' + ')} — ` +
			'required to type-check the built packages (deno task typecheck:packages); ' +
			'run `deno task bench:install`'
	);
	Deno.exit(1);
}

// The N-API half of the release is a different machine's: the tag Step 8 pushes triggers
// release.yml, AFTER this script has published the wasm packages — so a matrix that
// fails then leaves a half-released version. Its dispatch dry run on this commit is the
// rehearsal that answers that ahead of time, and nothing here can ask GitHub for it.
console.log(
	'  REMINDER: the release.yml workflow_dispatch dry run must be green on this commit ' +
		'before a --wetrun (the wasm packages publish before the tag starts the N-API matrix)'
);

// Registry. A fresh release's version must be unpublished everywhere; a resume skips what
// is already out, and one whose packages are all out only has git left to finish.
let published_count = 0;
if (release_version !== null) {
	for (const { label } of packages) {
		const published = await npm_is_published(label, release_version);
		if (!published.ok) failures.push(published.error);
		else if (published.published) {
			if (plan.kind === 'fresh') failures.push(`${label}@${release_version} is already on npm`);
			else published_count++;
		}
	}
}
const all_published = published_count === packages.length;

if (failures.length > 0 || refusals.length > 0) {
	for (const message of [...failures, ...refusals]) console.error(`  FAIL: ${message}`);
	console.error(
		`  Refusing to ${wetrun ? 'publish' : 'run'} — nothing was changed, built, or published.`
	);
	Deno.exit(1);
}

// Step 2: Bump

/** The version the build stamps: the bumped one under --wetrun, the checkout's in a dry run. */
let version: string = version_before;
/** Set once the bump is written: from then on a failure names the resume. */
let bumped = false;

if (wetrun) {
	console.log('\n=== Step 2: Bump ===');
	if (release_version === null) {
		console.error('  FAIL: no release version — the plan should have refused');
		Deno.exit(1);
	}
	if (plan.kind === 'fresh') {
		if (plan.stale_sentinel !== null) {
			console.warn(`  WARN: removing stale sentinel (v${plan.stale_sentinel})`);
			await sentinel_remove();
		}
		const written = await bump_cargo_version(release_version);
		if (!written.ok) {
			console.error(`  FAIL: ${written.error}`);
			Deno.exit(1);
		}
		// the sentinel right after the version write: everything below is idempotent and
		// re-runs on a resume, so any later failure is resumable
		await sentinel_write(release_version);
		console.log(`  Version bumped (${plan.level}): ${plan.version} -> ${release_version}`);
	} else {
		console.log(`  Resuming v${release_version} — bump already applied`);
	}
	bumped = true;
	version = release_version;
	const locked = await bump_lockfile();
	if (!locked.ok) fail_resumable(locked.error);
	const stamped = await bump_changelog(release_version);
	if (stamped === 'no_unreleased') {
		fail_resumable(`no "## Unreleased" or "## ${release_version}" in ${CHANGELOG_PATH}`);
	}
	console.log(
		stamped === 'already_stamped'
			? `  ${CHANGELOG_PATH} already stamped with ## ${release_version}`
			: `  Stamped ${CHANGELOG_PATH}: ## Unreleased -> ## ${release_version}; seeded fresh ## Unreleased (bump: patch)`
	);
} else {
	console.log('\n=== Step 2: Bump (dry-run — nothing written) ===');
	if (plan.kind === 'resume') console.log(`  A wetrun resumes v${plan.version}`);
	else if (plan.target !== null) console.log(`  A wetrun bumps to v${plan.target}`);
}

if (all_published) {
	console.log(`\n  Every package is already on npm at v${version} — only git remains.`);
	if (!wetrun) {
		console.log('  Dry-run complete — `deno task publish --wetrun` finishes the release.\n');
		Deno.exit(0);
	}
	await finalize_git();
	console.log('');
	Deno.exit(0);
}

/** The checkout as the build reads it; Step 6 compares against it before anything publishes. */
const checkout_before = await git_checkout_fingerprint();

// Step 3: Check

if (no_check) {
	console.log('\n=== Step 3: Check — SKIPPED (--no-check) ===');
} else {
	console.log('\n=== Step 3: Check (deno task check) ===');
	await run('deno task check', 'deno', ['task', 'check']);
	// The depth-limit stack contract in the shipped optimization: `check`'s `test:depth`
	// grades the `corpus` world, whose frames are not the shipped builds' (deno.json
	// `//test:depth:napi`).
	console.log('\n=== Step 3: Depth contract, shipped build (deno task test:depth:napi) ===');
	await run('deno task test:depth:napi', 'deno', ['task', 'test:depth:napi']);
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
	console.log('\n=== Step 3b: Conformance gates — SKIPPED (--no-check) ===');
	conformance_skip_reason = '--no-check';
} else {
	console.log('\n=== Step 3b: Conformance gates (deno task conformance:all) ===');
	const missing = [
		exists('../svelte/packages/svelte/tests') ? null : '../svelte checkout',
		exists('../acorn-typescript/test') ? null : '../acorn-typescript checkout',
		exists('../typescript/tests') ? null : '../typescript checkout',
		exists('../test262/test') ? null : '../test262 checkout',
		exists('../prettier/tests/format') ? null : '../prettier checkout',
		exists('../prettier-plugin-svelte/test') ? null : '../prettier-plugin-svelte checkout',
		exists(CORPORA_COLLECTIONS) ? null : `${CORPORA_ROOT} checkout (the real-code snapshot)`,
		exists('benches/js/node_modules') ? null : 'benches/js/node_modules (deno task bench:install)'
	].filter((m): m is string => m !== null);
	if (missing.length > 0 && wetrun) {
		console.error(
			`  FAIL: missing ${missing.join(' + ')} — the conformance gates cannot run. ` +
				'Clone/install the missing pieces, or pass --no-check to release without gates (explicitly).'
		);
		Deno.exit(1);
	} else if (missing.length > 0) {
		conformance_skip_reason = `missing ${missing.join(' + ')}`;
		console.warn(
			`  WARN: skipping — ${conformance_skip_reason}. ` +
				'A --wetrun would FAIL here; run `deno task conformance:all` on a machine with the oracles.'
		);
	} else {
		await run('deno task conformance:all', 'deno', ['task', 'conformance:all'], {
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
	console.log('\n=== Step 3c: Corpus robustness audit — SKIPPED (--no-check) ===');
	audit_corpus_skip_reason = '--no-check';
} else {
	console.log('\n=== Step 3c: Corpus robustness audit (deno task audit:corpus) ===');
	if (!exists(CORPORA_COLLECTIONS)) {
		audit_corpus_skip_reason = `missing ${CORPORA_ROOT} checkout (the real-code snapshot — the corpus floor)`;
		if (wetrun) {
			console.error(
				`  FAIL: ${audit_corpus_skip_reason} — the corpus audit cannot run over real code. ` +
					'Clone it, or pass --no-check to release without the audit (explicitly).'
			);
			Deno.exit(1);
		}
		console.warn(
			`  WARN: skipping — ${audit_corpus_skip_reason}. ` +
				'A --wetrun would FAIL here; run `deno task audit:corpus` on a machine with the corpus.'
		);
	} else {
		await run('deno task audit:corpus', 'deno', ['task', 'audit:corpus'], {
			fail_hint:
				'  Corpus robustness audit found a content-loss / panic / non-idempotency regression over\n' +
				'  REAL code (what `deno task check`’s fixture-only gate cannot see). Triage the failing\n' +
				'  leg by name with the matching `tsv_debug` audit on the single repo it points at.'
		});
	}
}

// Step 4: Build

console.log('\n=== Step 4: Build WASM bundles (npm + deno) ===');
// Single source of truth for the publishable artifact build: the `build:packages`
// task in deno.json. It builds everything Step 6 validates, including the
// unpublished deno bundles (stale bundles must never gate or falsely pass a
// publish), and is ordered to group cargo feature sets (format, format, parse,
// parse, default, default) so the wasm crate compiles once per feature set, not
// once per task. CI's artifacts job runs the same task, so the two can't drift.
// A release build must never freshness-skip: the wasm build tasks ride
// scripts/run_if_stale.ts, whose mtime check can't see toolchain changes
// (wasm-pack/wasm-opt/rustc upgrades). Force for the whole build step.
await run('deno task build:packages', 'deno', ['task', 'build:packages'], {
	env: { TSV_BUILD_FORCE: '1' }
});

// Step 5: Verify built packages

console.log('\n=== Step 5: Verify built package name + version ===');
for (const { label, dir } of packages) {
	const built_pkg = JSON.parse(Deno.readTextFileSync(`${dir}/package.json`));
	if (built_pkg.name !== label) {
		fail_resumable(`${dir} built as ${built_pkg.name}, expected ${label}`);
	}
	if (built_pkg.version !== version) {
		fail_resumable(`${label} built at v${built_pkg.version}, expected v${version}`);
	}
	console.log(`  PASS: ${label} = v${version}`);
}

// Step 6: Test the built artifacts

console.log('\n=== Step 6: Validate built packages (sizes + Deno + Node + Bun + declarations) ===');
await run('deno task validate:artifacts', 'deno', ['task', 'validate:artifacts']);
// A second spelling of deno.json's `test:npm*:run` tasks (see `//test:npm:run`): a leg added
// there must be added here too, or a publish silently skips it.
for (const { label, dir } of packages) {
	await run(`artifact tests for ${label}`, 'node', ['--test', 'scripts/test_npm.ts'], {
		env: { PKG_DIR: dir }
	});
}
// The depth limits' stack contract in the WASM engine, over the `all` package just built:
// every shape the native gate generates, at its accepted depth, on a Node main thread, cold
// and tiered up (deno.json `//test:depth:wasm`). A `check`-cadence gate like Step 3's
// shipped-build leg, so `--no-check` skips it with that one.
if (no_check) {
	console.warn('  WARN: skipping deno task test:depth:wasm (--no-check)');
} else {
	await run('deno task test:depth:wasm', 'deno', ['task', 'test:depth:wasm']);
}
// The parse-failure table under Bun, the one runtime that grades the engines' point
// properties (deno.json `//test:bun`), over the three wasm packages this script publishes —
// the napi engine under Bun is graded locally by `test:napi:npm`, not here. Bun's presence
// was settled by the preflight, so the leg requires it.
if (bun_skip_reason === null) {
	await run('deno task test:bun', 'deno', [
		'task',
		'test:bun',
		'--require-bun',
		'format',
		'parse',
		'all'
	]);
} else {
	console.warn(`  WARN: skipping deno task test:bun — ${bun_skip_reason}`);
}
// The merged published declarations, as a consumer's compiler sees them, for every npm
// package this release ships. The napi loader publishes from release.yml, but its
// staging is a file copy (no cargo build), so it is staged here at this version and graded
// beside the three wasm packages — all four named, so each is required. Needs the
// benches/js install whatever --no-check says: it grades what is about to ship.
await run('stage the @fuzdev/tsv loader', 'deno', [
	'run',
	'--allow-read',
	'--allow-write=crates/tsv_napi/pkg',
	'scripts/build_napi_packages.ts',
	'--loader-only'
]);
await run('deno task typecheck:packages', 'deno', [
	'task',
	'typecheck:packages',
	'format',
	'parse',
	'all',
	'napi'
]);

// The packages hold what the checkout held while Step 4 built them: a checkout that moved
// since the bump refuses a wetrun rather than publish packages no commit describes.
const checkout_after = await git_checkout_fingerprint();
if (
	!checkout_before.ok ||
	!checkout_after.ok ||
	checkout_before.fingerprint !== checkout_after.fingerprint
) {
	const moved = 'the checkout changed while the gates ran — the built packages may match no commit';
	if (wetrun) fail_resumable(`${moved}; refusing to publish`);
	warnings++;
	console.warn(`  WARN: ${moved}`);
}

// Step 7: Publish

console.log('\n=== Step 7: Publish to npm ===');

// Publish loop is idempotent: already-published packages are skipped, so retry after
// partial publish works automatically.

for (let i = 0; i < packages.length; i++) {
	const { label, dir } = packages[i];
	if (wetrun) {
		const published = await npm_is_published(label, version);
		if (!published.ok) fail_resumable(published.error);
		if (published.published) {
			console.log(`  SKIP: ${label}@${version} already published`);
			continue;
		}
		console.log(`  Publishing ${label}...`);
		const not_published = packages.slice(i);
		const fail_hint = `  Packages not yet published (a resume retries them, or publish by hand):\n${not_published
			.map((p) => `    cd ${p.dir} && npm publish --access public`)
			.join('\n')}`;
		await run(`npm publish ${label}`, 'npm', ['publish', '--access', 'public'], {
			cwd: dir,
			fail_hint
		});
		console.log(`  Published ${label}@${version}`);
	} else {
		console.log(`  [dry-run] ${label}:`);
		const res = await command_capture('npm', ['publish', '--dry-run', '--access', 'public'], {
			cwd: dir
		});
		// npm prints the tarball notice to stderr — surface it regardless.
		if (res.err) console.log(res.err);
		if (res.success) {
			console.log(`  PASS: ${label} packs cleanly`);
		} else if (plan.kind === 'fresh' && npm_error_is_republish(res.err)) {
			// Dry-run never bumps, so it dry-publishes the CURRENT (already-published)
			// version. The real wetrun bumps first, so this isn't a real failure.
			const target = plan.target === null ? 'the bumped version' : `v${plan.target}`;
			console.log(
				`  PASS: ${label} packs cleanly (v${version} already published — a real run publishes ${target})`
			);
		} else {
			if (res.out) console.error(res.out);
			console.error(`  FAIL: npm publish --dry-run ${label}`);
			Deno.exit(1);
		}
	}
}

// Step 8: Git finalize

if (wetrun) await finalize_git();

// Summary

console.log('\n=== Done ===');
for (const { label, dir } of packages) {
	const wasm_path = `${dir}/tsv_wasm_bg.wasm`;
	const raw = Deno.statSync(wasm_path).size;
	const gzipped = await gzip_size(wasm_path);
	const size_note = `wasm ${format_size(raw)} (${format_size(gzipped)} gzipped wire)`;
	console.log(
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
	console.log(
		no_git
			? `\n  The N-API set does NOT publish until the v${version} tag is pushed —`
			: `\n  The pushed v${version} tag triggers release.yml —`
	);
	console.log('  the N-API set (@fuzdev/tsv + its platform packages) publishes there.');
	console.log(
		no_git
			? '  Finish the finalize above, then watch that run.'
			: '  The release is complete only once that run is green; if the matrix fails, re-run it\n' +
					'  from the Actions tab — dispatching ON THE TAG, so the version assertion still holds.'
	);
}
if (conformance_skip_reason !== null) {
	// Re-warn at the very end — the Step 3b warning is a single line buried in
	// build output, and a release whose conformance gates never ran must be the
	// last thing on screen, not a scrolled-away footnote.
	console.warn(`\n  ⚠ Step 3b conformance gates did NOT run (${conformance_skip_reason}).`);
	console.warn('    Run `deno task conformance:all` and verify it passes for this release.');
}
if (audit_corpus_skip_reason !== null) {
	// Re-warn at the very end — same reasoning as the Step 3b re-warn: a release whose real-code
	// robustness audit never ran must be the last thing on screen, not a scrolled-away footnote.
	console.warn(`\n  ⚠ Step 3c corpus robustness audit did NOT run (${audit_corpus_skip_reason}).`);
	console.warn('    Run `deno task audit:corpus` and verify it passes for this release.');
}
if (bun_skip_reason !== null) {
	// Re-warn at the very end, beside the Step 3b/3c re-warns: the engines' Bun-only point
	// properties went ungraded for this release.
	console.warn(`\n  ⚠ Step 6 parse-failure table under Bun did NOT run (${bun_skip_reason}).`);
	console.warn(
		'    Run `deno task test:bun format parse all` with bun installed for this release.'
	);
}
if (!wetrun) {
	if (warnings > 0) {
		console.log(
			`\n  Dry-run complete for v${version} — a wetrun would FAIL on the warnings above.`
		);
	} else if (plan.kind === 'resume') {
		console.log(`\n  Dry-run complete for v${version} — all checks passed.`);
		console.log('  Run `deno task publish --wetrun` to resume the release.');
	} else {
		console.log(`\n  Dry-run complete for v${version} — all checks passed.`);
		console.log(`  Run with --wetrun --bump ${plan.level} to publish v${plan.target}.`);
	}
}
console.log('');

// Helpers

/** Step 8: commit exactly the release files, tag the commit, push both, then remove the
 * sentinel — or, under `--no-git`, print those steps. Past Step 7 the packages are public,
 * so a failure here is the one place a bare exit code is not enough: it names the exact
 * recovery. Every step no-ops when already done, so a resume repeats it safely. */
async function finalize_git(): Promise<void> {
	// every dry-run path exits before this; it guards a future edit that falls through
	if (!wetrun) {
		console.error('  FAIL: a dry run reached the git finalize — refusing to commit, tag, or push');
		Deno.exit(1);
	}
	const tag = version_tag(version);
	const message = release_commit_message(version);
	const manual_finalize = [
		`    git add ${RELEASE_PATHS.join(' ')}`,
		`    git commit -m "${message}"`,
		`    git tag -m ${tag} ${tag}`,
		'    git push --follow-tags origin main'
	].join('\n');
	function git_fail(error: string): never {
		console.error(`\n  FAIL: ${error}`);
		console.error(
			'  The packages are PUBLISHED — only the release commit + tag + push is missing.\n' +
				'  The sentinel survives this step, so `deno task publish --wetrun` resumes from here\n' +
				'  (it skips straight to this step once every package is on npm). Or finalize by hand:\n' +
				manual_finalize
		);
		Deno.exit(1);
	}

	if (no_git) {
		console.log('\n=== Step 8: Git finalize — SKIPPED (--no-git) ===');
		console.log('  Finalize manually (only the release files; annotated tag — --follow-tags');
		console.log('  ignores lightweight tags):');
		console.log(manual_finalize);
	} else {
		console.log('\n=== Step 8: Git finalize (commit + tag + push) ===');
		const committed = await git_commit_paths(RELEASE_PATHS, message);
		if (!committed.ok) git_fail(committed.error);
		console.log(committed.committed ? `  Committed "${message}"` : '  Release commit already made');
		if ((await git_local_tag_commit(tag)) === null) {
			const tagged = await git_tag_annotated(tag);
			if (!tagged.ok) git_fail(tagged.error);
			console.log(`  Tagged ${tag}`);
		} else console.log(`  Tag ${tag} already exists`);
		// this push is what triggers release.yml — until it lands, the wasm packages
		// are public with no tag, so the N-API set never builds
		if (!(await git_push_main_with_tags())) git_fail('git push --follow-tags origin main failed');
	}

	// last: the release is complete only once the commit, tag and push have landed (or
	// --no-git handed them off); removing it earlier would make a finalize failure
	// unresumable, since the fresh path refuses the `## Unreleased` this run already stamped
	await sentinel_remove();
}

/** Exit on a failure, naming the resume once the bump is in place. */
function fail_resumable(message: string): never {
	console.error(`  FAIL: ${message}`);
	exit_resumable();
}

function exit_resumable(): never {
	if (bumped) {
		console.error(
			'  The bump is in place — fix the cause and run `deno task publish --wetrun` to resume.'
		);
	}
	Deno.exit(1);
}

/** A tool's `--version` output, or `null` when it is not installed or fails. */
async function probe_version(cmd: string): Promise<string | null> {
	const result = await command_capture(cmd, ['--version']);
	return result.success ? result.out.trim() : null;
}

function exists(path: string): boolean {
	try {
		Deno.statSync(path);
		return true;
	} catch (error) {
		if (error instanceof Deno.errors.NotFound) return false;
		throw error;
	}
}

/** Run a long step with the terminal attached; exit on failure, printing `fail_hint` and,
 * after the bump, the resume under the failure. `env` adds to the inherited environment. */
async function run(
	label: string,
	cmd: string,
	args: ReadonlyArray<string>,
	options: CommandOptions & { fail_hint?: string } = {}
): Promise<void> {
	const { fail_hint, ...command_options } = options;
	const result = await command_stream(cmd, args, command_options);
	if (result.success) return;
	console.error(`\n  FAIL: ${label} (exit code ${result.code})`);
	if (fail_hint) console.error(fail_hint);
	exit_resumable();
}

/** The changelog text, or null when there is no CHANGELOG.md. Only a missing
 * file reads as null — a permission error or the like is a real failure, not
 * "no changelog", and rethrows. */
function read_changelog(): string | null {
	try {
		return Deno.readTextFileSync(CHANGELOG_PATH);
	} catch (error) {
		if (error instanceof Deno.errors.NotFound) return null;
		throw error;
	}
}

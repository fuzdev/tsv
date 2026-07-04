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
 * section into the new version's section.
 *
 * Usage:
 *   deno task publish                        # dry-run (validate everything)
 *   deno task publish --bump minor           # dry-run, preview the bump
 *   deno task publish --wetrun --bump patch  # bump + check + conformance + build + validate + publish + git
 *   deno task publish --wetrun               # resume a failed wetrun (sentinel retry only)
 *
 * Flags:
 *   --wetrun         actually mutate: bump, publish, commit, tag, push
 *   --bump <level>   patch | minor | major — how to bump the version. Required,
 *                    and must match the CHANGELOG `## Unreleased` `<!-- bump:
 *                    <level> -->` marker (also required). A fresh wetrun needs
 *                    both, plus a non-empty `## Unreleased` section; a sentinel
 *                    retry runs without either.
 *   --no-check       skip `deno task check` AND the Step 3b conformance gates (faster retries)
 *   --no-git         skip the git commit + tag + push finalization
 *
 * Retry: a failed wetrun leaves the bump in place plus a sentinel file; re-run
 * `deno task publish --wetrun` and it resumes from the bumped version (skipping
 * the git-clean check and the bump; a passed `--bump` is ignored with a
 * warning). The publish loop itself is idempotent — already-published packages
 * are skipped.
 */

import { format_size, gzip_size } from './size.ts';

const KNOWN_FLAGS = new Set(['--wetrun', '--no-check', '--no-git', '--bump']);
// A misspelled --wetrun fails safe (dry-run), but a misspelled --no-git or
// --no-check fails open — reject anything unrecognized.
const unknown_args = Deno.args.filter((arg, i) =>
	arg.startsWith('--') ? !KNOWN_FLAGS.has(arg) : Deno.args[i - 1] !== '--bump'
);
if (unknown_args.length > 0) {
	console.error(`FAIL: unknown argument(s): ${unknown_args.join(' ')}`);
	console.error(
		'Usage: deno task publish [--wetrun] [--bump patch|minor|major] [--no-check] [--no-git]',
	);
	Deno.exit(1);
}

const wetrun = Deno.args.includes('--wetrun');
const no_check = Deno.args.includes('--no-check');
const no_git = Deno.args.includes('--no-git');
const bump_index = Deno.args.indexOf('--bump');
const bump = bump_index === -1 ? null : Deno.args[bump_index + 1];
if (bump !== null && bump !== 'patch' && bump !== 'minor' && bump !== 'major') {
	console.error(`FAIL: --bump must be patch, minor, or major (got ${JSON.stringify(bump)})`);
	Deno.exit(1);
}

type BumpLevel = 'patch' | 'minor' | 'major';

const SENTINEL_PATH = '.publish-in-progress';
const CARGO_PATH = 'Cargo.toml';
const CHANGELOG_PATH = 'CHANGELOG.md';
// Match version under [workspace.package] to avoid clobbering dependency versions
const workspace_pkg_re = /(\[workspace\.package\][\s\S]*?^version\s*=\s*)"([^"]*)"/m;

/** The `<!-- bump: <level> -->` marker, recognized ONLY on the line directly
 * after the `## Unreleased` heading — the exact position `stamp_changelog`'s
 * `with_marker` rewrites. `unreleased_section` returns the body starting at the
 * newline after the heading, so the leading `\n` here anchors that placement.
 * A marker anywhere else in the section is ignored, so validation can't accept
 * a changelog whose marker the stamper would fail to strip. Kept in sync with
 * `with_marker`. Declared here (not beside the changelog helpers below) so it's
 * initialized before the top-level orchestration calls `changelog_declared_bump`. */
const UNRELEASED_BUMP_MARKER = /^\n<!-- bump: (patch|minor|major) -->(?:\n|$)/;

const dec = new TextDecoder();

/** The published packages, in publish order. */
const packages = [
	{
		label: '@fuzdev/tsv_format_wasm',
		dir: 'crates/tsv_wasm/pkg/format/npm',
	},
	{
		label: '@fuzdev/tsv_parse_wasm',
		dir: 'crates/tsv_wasm/pkg/parse/npm',
	},
	{
		label: '@fuzdev/tsv_wasm',
		dir: 'crates/tsv_wasm/pkg/all/npm',
	},
];

/** Only the release files — a stray file generated mid-pipeline must never
 * ride into the release commit. */
const release_files = [CARGO_PATH, 'Cargo.lock', CHANGELOG_PATH];

console.log(`\n=== tsv publish ${wetrun ? '(wetrun)' : '(dry-run)'} ===\n`);

// Step 1: Preflight checks

console.log('=== Step 1: Preflight checks ===');

const version_before = read_cargo_version();
if (!version_before) {
	console.error('  FAIL: could not find [workspace.package] version in Cargo.toml');
	Deno.exit(1);
}

const initial_sentinel = read_sentinel();
// Only treat a *matching* sentinel as a retry — stale sentinels (wrong version) get removed below
const retry_mode = wetrun && initial_sentinel !== null && initial_sentinel === version_before;

// Branch check — publish from main only
const branch_result = capture('git', ['branch', '--show-current']);
if (!branch_result.success) {
	console.error('  FAIL: git branch failed — is this a git repository?');
	Deno.exit(1);
}
const branch = branch_result.stdout;
if (branch !== 'main') {
	if (wetrun) {
		console.error(`  FAIL: on branch "${branch}" — publish from main`);
		Deno.exit(1);
	}
	console.warn(`  WARN: on branch "${branch}" — wetrun requires main`);
} else {
	console.log('  Branch: main');
}

// Origin sync — the final `git push` runs after npm publish, when a stale
// main is no longer recoverable by re-running. Catch it up front instead.
if (branch === 'main') {
	if (!capture('git', ['fetch', 'origin', 'main']).success) {
		if (wetrun) {
			console.error('  FAIL: git fetch origin main failed — cannot verify sync with origin');
			Deno.exit(1);
		}
		console.warn('  WARN: git fetch origin main failed — skipping origin sync check (dry-run)');
	} else {
		const behind_result = capture('git', ['rev-list', '--count', 'HEAD..origin/main']);
		const behind = Number(behind_result.stdout);
		if (!behind_result.success || Number.isNaN(behind)) {
			console.error('  FAIL: could not compare HEAD with origin/main');
			Deno.exit(1);
		}
		if (behind > 0) {
			if (wetrun) {
				console.error(
					`  FAIL: main is ${behind} commit(s) behind origin/main — run \`git pull\` first`,
				);
				Deno.exit(1);
			}
			console.warn(`  WARN: main is ${behind} commit(s) behind origin/main — wetrun would fail`);
		} else {
			console.log('  Origin: up to date with origin/main');
		}
	}
}

// Git cleanliness — skipped in retry mode (the bump left changes behind)
if (retry_mode) {
	console.log(`  Sentinel detected at v${version_before} — skipping git cleanliness check (retry)`);
} else {
	const git_status = capture('git', ['status', '--porcelain']);
	if (!git_status.success) {
		console.error('  FAIL: git status failed');
		Deno.exit(1);
	}
	const uncommitted = git_status.stdout;
	if (uncommitted) {
		console.warn('  WARN: uncommitted changes in worktree:');
		for (const line of uncommitted.split('\n')) {
			console.warn(`    ${line}`);
		}
		if (wetrun) {
			console.error('  FAIL: refusing to publish with uncommitted changes');
			Deno.exit(1);
		}
		console.warn('  Continuing dry-run anyway...');
	}
}

// npm auth (only for wetrun — no point failing dry-runs over auth)
if (wetrun) {
	const whoami = capture('npm', ['whoami']);
	if (!whoami.success) {
		console.error('  FAIL: not logged in to npm (run `npm login` first)');
		Deno.exit(1);
	}
	console.log(`  npm authenticated as: ${whoami.stdout}`);
}

// node is required for the artifact tests
const node_check = capture('node', ['--version']);
if (!node_check.success) {
	console.error('  FAIL: node not found — required to test the built packages');
	Deno.exit(1);
}
console.log(`  node: ${node_check.stdout}`);

// Step 2: Resolve version

let version: string;

if (wetrun) {
	console.log('\n=== Step 2: Resolve version ===');
	if (retry_mode) {
		version = version_before;
		console.log(`  Sentinel found at v${version} — skipping bump (retry)`);
		if (bump) {
			console.warn(`  WARN: --bump ${bump} ignored — resuming the in-progress v${version} release`);
		}
	} else {
		if (initial_sentinel !== null) {
			console.warn(`  WARN: removing stale sentinel (v${initial_sentinel})`);
			Deno.removeSync(SENTINEL_PATH);
		}
		// A release must ship notes — require a non-empty `## Unreleased`
		// section before mutating anything (the bump level can live there too).
		const content = changelog_unreleased_content();
		if (content === null) {
			console.error(
				`  FAIL: no "## Unreleased" section in ${CHANGELOG_PATH} — add release notes before publishing`,
			);
			Deno.exit(1);
		}
		if (content === '') {
			console.error(
				`  FAIL: "## Unreleased" section in ${CHANGELOG_PATH} is empty — add release notes before publishing`,
			);
			Deno.exit(1);
		}
		const level = resolve_bump(bump as BumpLevel | null, changelog_declared_bump());
		version = bump_version(version_before, level);
		const cargo = Deno.readTextFileSync(CARGO_PATH);
		Deno.writeTextFileSync(CARGO_PATH, cargo.replace(workspace_pkg_re, `$1"${version}"`));
		console.log(`  Version bumped (${level}): ${version_before} -> ${version}`);
		// Sentinel immediately after the version write — everything below is
		// idempotent and re-runs on retry, so any later failure is resumable.
		Deno.writeTextFileSync(SENTINEL_PATH, version);
		console.log(`  Sentinel written (${SENTINEL_PATH})`);
	}
	// Idempotent bump finalization — re-run on retry too, in case the previous
	// wetrun died partway through.
	// Sync Cargo.lock's workspace member versions
	run('cargo update --workspace', 'cargo', ['update', '--workspace']);
	stamp_changelog(version);
} else {
	console.log('\n=== Step 2: Read version (dry-run) ===');
	version = version_before;
	console.log(`  Current version: v${version}`);
	const would_retry = initial_sentinel === version;
	const declared = changelog_declared_bump();
	if (would_retry) {
		// Mirrors wetrun precedence: retry wins over --bump
		console.log(`  Sentinel found at v${version} — wetrun would retry from it`);
		if (bump) {
			console.warn(`  WARN: --bump ${bump} would be ignored — wetrun would resume v${version}`);
		}
	} else {
		// Mirror the wetrun's requirements without exiting — report what it would do.
		const content = changelog_unreleased_content();
		if (content === null) {
			console.warn(`  WARN: no "## Unreleased" section in ${CHANGELOG_PATH} — a fresh wetrun would FAIL`);
		} else if (content === '') {
			console.warn(`  WARN: "## Unreleased" section is empty — a fresh wetrun would FAIL`);
		}
		if (!bump) {
			console.warn(
				'  WARN: no --bump given — a fresh wetrun would FAIL (required, and must match the CHANGELOG marker)',
			);
		}
		if (!declared) {
			console.warn(
				`  WARN: no "<!-- bump: <level> -->" marker in ${CHANGELOG_PATH} — a fresh wetrun would FAIL`,
			);
		}
		if (bump && declared && bump !== declared) {
			console.warn(
				`  WARN: --bump ${bump} disagrees with CHANGELOG marker <!-- bump: ${declared} --> — a fresh wetrun would FAIL`,
			);
		}
		if (bump && declared && bump === declared) {
			console.log(`  Wetrun would bump (${bump}) to: v${bump_version(version, bump)}`);
		}
	}
}

if (!/^\d+\.\d+\.\d+$/.test(version)) {
	console.error(`  FAIL: version "${version}" does not look like semver`);
	Deno.exit(1);
}

// Step 3: Check

if (no_check) {
	console.log('\n=== Step 3: Check — SKIPPED (--no-check) ===');
} else {
	console.log('\n=== Step 3: Check (deno task check) ===');
	run('deno task check', 'deno', ['task', 'check']);
}

// Step 3b: Conformance gates
//
// The release-cadence correctness gates that run against EXTERNAL oracles and so
// can't live in `deno task check`: the Svelte parser vs `svelte/compiler`
// (conformance:svelte-fixtures), and all three languages' AST + formatter vs the
// canonical parsers / prettier (corpus:compare:parse + :format, --all). Running
// them here means a release can't ship a parse/format regression the in-repo gate
// is structurally blind to. Skipped by --no-check alongside Step 3.
//
// Tolerant of a missing oracle: these need the ../svelte checkout + the
// benches/js `node_modules` sidecar (`deno task bench:install`), which a clean
// machine or a resumed wetrun may lack — warn + skip rather than block the
// release. test262 (needs ../test262) and the CSS-WPT harvest stay manual.
if (no_check) {
	console.log('\n=== Step 3b: Conformance gates — SKIPPED (--no-check) ===');
} else {
	console.log('\n=== Step 3b: Conformance gates (deno task conformance) ===');
	const missing = [
		exists('../svelte/packages/svelte/tests') ? null : '../svelte checkout',
		exists('benches/js/node_modules') ? null : 'benches/js/node_modules (deno task bench:install)',
	].filter((m): m is string => m !== null);
	if (missing.length > 0) {
		console.warn(
			`  WARN: skipping — missing ${missing.join(' + ')}. ` +
				'Run `deno task conformance` manually before release.',
		);
	} else {
		run(
			'deno task conformance',
			'deno',
			['task', 'conformance'],
			undefined,
			'  Conformance gate failed. If it was a corpus:compare:format SAFETY hit, re-run\n' +
				'  `deno task corpus:compare:format ~/dev/<that-repo>` on the single repo to rule out\n' +
				'  the known --all FFI heisenbug (benches/js/CLAUDE.md §Known Issues) before treating\n' +
				'  it as a real regression.',
		);
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
run('deno task build:packages', 'deno', ['task', 'build:packages']);

// Step 5: Verify built packages

console.log('\n=== Step 5: Verify built package name + version ===');
for (const { label, dir } of packages) {
	const built_pkg = JSON.parse(Deno.readTextFileSync(`${dir}/package.json`));
	if (built_pkg.name !== label) {
		console.error(`  FAIL: ${dir} built as ${built_pkg.name}, expected ${label}`);
		Deno.exit(1);
	}
	if (built_pkg.version !== version) {
		console.error(`  FAIL: ${label} built at v${built_pkg.version}, expected v${version}`);
		Deno.exit(1);
	}
	console.log(`  PASS: ${label} = v${version}`);
}

// Step 6: Test the built artifacts

console.log('\n=== Step 6: Validate built packages (sizes + Deno + Node) ===');
run('deno task validate:artifacts', 'deno', ['task', 'validate:artifacts']);
for (const { label, dir } of packages) {
	const result = new Deno.Command('node', {
		args: ['--test', 'scripts/test_npm.ts'],
		env: { PKG_DIR: dir },
		stdin: 'inherit',
		stdout: 'inherit',
		stderr: 'inherit',
	}).outputSync();
	if (!result.success) {
		console.error(`\n  FAIL: artifact tests for ${label}`);
		Deno.exit(1);
	}
}

// Step 7: Publish

console.log('\n=== Step 7: Publish to npm ===');

// Publish loop is idempotent: already-published packages are skipped, so retry after
// partial publish works automatically. Sentinel is removed after all succeed.

for (let i = 0; i < packages.length; i++) {
	const { label, dir } = packages[i];
	if (wetrun) {
		if (is_published(label, version)) {
			console.log(`  SKIP: ${label}@${version} already published`);
			continue;
		}
		console.log(`  Publishing ${label}...`);
		const not_published = packages.slice(i);
		const fail_hint =
			`  Packages not published — re-run \`deno task publish --wetrun\` to retry, or publish manually:\n${
				not_published.map((p) => `    cd ${p.dir} && npm publish --access public`).join('\n')
			}`;
		run(`npm publish ${label}`, 'npm', ['publish', '--access', 'public'], dir, fail_hint);
		console.log(`  Published ${label}@${version}`);
	} else {
		console.log(`  [dry-run] ${label}:`);
		const res = capture('npm', ['publish', '--dry-run', '--access', 'public'], dir);
		// npm prints the tarball notice to stderr — surface it regardless.
		if (res.stderr) console.log(res.stderr);
		if (res.success) {
			console.log(`  PASS: ${label} packs cleanly`);
		} else if (/cannot publish over|previously published version/i.test(res.stderr)) {
			// Dry-run never bumps, so it dry-publishes the CURRENT (already-published)
			// version. The real wetrun bumps first, so this isn't a real failure.
			const preview = (bump as BumpLevel | null) ?? changelog_declared_bump();
			const target = preview ? `v${bump_version(version, preview)}` : 'the bumped version';
			console.log(
				`  PASS: ${label} packs cleanly (v${version} already published — a real run publishes ${target})`,
			);
		} else {
			if (res.stdout) console.error(res.stdout);
			console.error(`  FAIL: npm publish --dry-run ${label}`);
			Deno.exit(1);
		}
	}
}

// Remove sentinel after all packages are published — retry is safe up to this point.
if (wetrun) {
	try {
		Deno.removeSync(SENTINEL_PATH);
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;
	}
}

// Step 8: Git finalize

if (wetrun && !no_git) {
	console.log('\n=== Step 8: Git finalize (commit + tag + push) ===');
	if (capture('git', ['status', '--porcelain', '--', ...release_files]).stdout) {
		run('git add', 'git', ['add', '--', ...release_files]);
		run('git commit', 'git', ['commit', '-m', `publish v${version}`]);
	} else {
		console.log('  Nothing to commit (retry after a previous commit)');
	}
	const tag = `v${version}`;
	if (capture('git', ['rev-parse', '-q', '--verify', `refs/tags/${tag}`]).success) {
		console.log(`  Tag ${tag} already exists`);
	} else {
		// Annotated — `git push --follow-tags` ignores lightweight tags
		run('git tag', 'git', ['tag', '-m', tag, tag]);
	}
	run('git push', 'git', ['push', '--follow-tags', 'origin', 'main']);
} else if (wetrun) {
	console.log('\n=== Step 8: Git finalize — SKIPPED (--no-git) ===');
	console.log('  Finalize manually (only the release files; annotated tag — --follow-tags');
	console.log('  ignores lightweight tags):');
	console.log(`    git add ${release_files.join(' ')}`);
	console.log(`    git commit -m "publish v${version}"`);
	console.log(`    git tag -m v${version} v${version}`);
	console.log('    git push --follow-tags origin main');
}

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
			: `  ${label}@${version} — ${size_note}`,
	);
}
if (!wetrun) {
	console.log(`\n  Dry-run complete for v${version} — all checks passed.`);
	console.log('  Run with --wetrun --bump patch|minor|major to publish.');
}
console.log('');

// Helpers

/** Run a command silently and return success + trimmed stdout/stderr. */
function capture(
	cmd: string,
	args: string[],
	cwd?: string,
): { success: boolean; stdout: string; stderr: string } {
	const result = new Deno.Command(cmd, {
		args,
		cwd,
		stdout: 'piped',
		stderr: 'piped',
	}).outputSync();
	return {
		success: result.success,
		stdout: dec.decode(result.stdout).trim(),
		stderr: dec.decode(result.stderr).trim(),
	};
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

function run(label: string, cmd: string, args: string[], cwd?: string, fail_hint?: string): void {
	const result = new Deno.Command(cmd, {
		args,
		cwd,
		stdin: 'inherit',
		stdout: 'inherit',
		stderr: 'inherit',
	}).outputSync();
	if (!result.success) {
		console.error(`\n  FAIL: ${label} (exit code ${result.code})`);
		if (fail_hint) console.error(fail_hint);
		Deno.exit(1);
	}
}

function bump_version(current: string, level: 'patch' | 'minor' | 'major'): string {
	const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
	if (!match) {
		console.error(`  FAIL: cannot bump non-semver version "${current}"`);
		Deno.exit(1);
	}
	const [major, minor, patch] = [Number(match[1]), Number(match[2]), Number(match[3])];
	if (level === 'major') return `${major + 1}.0.0`;
	if (level === 'minor') return `${major}.${minor + 1}.0`;
	return `${major}.${minor}.${patch + 1}`;
}

/** Stamp CHANGELOG.md's `## Unreleased` section into `## <version>` (dropping its
 * bump marker) and seed a fresh empty `## Unreleased` reset to `bump: patch` for
 * the next cycle. Idempotent: a no-op if `## <version>` is already present. */
function stamp_changelog(new_version: string): void {
	let changelog: string;
	try {
		changelog = Deno.readTextFileSync(CHANGELOG_PATH);
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;
		console.warn(`  WARN: no ${CHANGELOG_PATH} — skipping changelog stamp`);
		return;
	}
	// Already stamped this version (retry after a failed wetrun) — the seeded
	// fresh `## Unreleased` is expected; leave everything as-is.
	if (version_heading_re(new_version).test(changelog)) {
		console.log(`  ${CHANGELOG_PATH} already stamped with ## ${new_version}`);
		return;
	}
	// Rename `## Unreleased` to the version (dropping its bump marker) and seed a
	// fresh empty `## Unreleased` reset to `bump: patch` for the next cycle.
	const fresh = '## Unreleased\n<!-- bump: patch -->\n\n';
	const with_marker = /^## Unreleased\n<!-- bump: (?:patch|minor|major) -->\n/m;
	if (with_marker.test(changelog)) {
		const stamped = changelog.replace(with_marker, `${fresh}## ${new_version}\n`);
		Deno.writeTextFileSync(CHANGELOG_PATH, stamped);
		console.log(
			`  Stamped ${CHANGELOG_PATH}: ## Unreleased -> ## ${new_version}; seeded fresh ## Unreleased (bump: patch)`,
		);
	} else if (/^## Unreleased$/m.test(changelog)) {
		// No marker (defensive — a wetrun would have failed earlier). Rename + seed.
		const stamped = changelog.replace(/^## Unreleased$/m, `${fresh}## ${new_version}`);
		Deno.writeTextFileSync(CHANGELOG_PATH, stamped);
		console.log(
			`  Stamped ${CHANGELOG_PATH}: ## Unreleased -> ## ${new_version}; seeded fresh ## Unreleased (bump: patch)`,
		);
	} else {
		console.warn(`  WARN: no "## Unreleased" section in ${CHANGELOG_PATH} — nothing to stamp`);
	}
}

function version_heading_re(target_version: string): RegExp {
	return new RegExp(`^## ${target_version.replaceAll('.', '\\.')}$`, 'm');
}

/** Body of the `## Unreleased` section (after the heading, up to the next `## `
 * heading or EOF), or null if there's no such heading. */
function unreleased_section(changelog: string): string | null {
	const heading = /^## Unreleased$/m.exec(changelog);
	if (!heading) return null;
	const after = changelog.slice(heading.index + heading[0].length);
	const next = after.search(/^## /m);
	return next === -1 ? after : after.slice(0, next);
}

/** Unreleased content with the bump marker + surrounding whitespace stripped.
 * `''` means an empty section; `null` means no section (or no CHANGELOG). */
function changelog_unreleased_content(): string | null {
	let changelog: string;
	try {
		changelog = Deno.readTextFileSync(CHANGELOG_PATH);
	} catch {
		return null;
	}
	const section = unreleased_section(changelog);
	if (section === null) return null;
	return section.replace(UNRELEASED_BUMP_MARKER, '').trim();
}

/** The `<!-- bump: <level> -->` marker declared directly after the `## Unreleased`
 * heading, or null. */
function changelog_declared_bump(): BumpLevel | null {
	let changelog: string;
	try {
		changelog = Deno.readTextFileSync(CHANGELOG_PATH);
	} catch {
		return null;
	}
	const section = unreleased_section(changelog);
	if (section === null) return null;
	const m = UNRELEASED_BUMP_MARKER.exec(section);
	return m ? (m[1] as BumpLevel) : null;
}

/** The bump level — required on BOTH the --bump flag and the CHANGELOG marker,
 * and they must match. Exits if either is missing or they disagree. */
function resolve_bump(flag: BumpLevel | null, declared: BumpLevel | null): BumpLevel {
	if (!flag) {
		console.error('  FAIL: --bump patch|minor|major is required (and must match the CHANGELOG marker)');
		Deno.exit(1);
	}
	if (!declared) {
		console.error(
			`  FAIL: no "<!-- bump: <level> -->" marker under ## Unreleased in ${CHANGELOG_PATH} — declare the bump there too`,
		);
		Deno.exit(1);
	}
	if (flag !== declared) {
		console.error(
			`  FAIL: --bump ${flag} disagrees with CHANGELOG marker <!-- bump: ${declared} --> (they must match)`,
		);
		Deno.exit(1);
	}
	return flag;
}

/**
 * Whether `pkg_name@target_version` exists on the registry. Distinguishes
 * "not published" (npm E404) from npm/network failure — the latter aborts,
 * since no publish decision is safe without an answer.
 */
function is_published(pkg_name: string, target_version: string): boolean {
	const result = capture('npm', ['view', `${pkg_name}@${target_version}`, 'version']);
	if (result.success) return result.stdout === target_version;
	if (result.stderr.includes('E404')) return false;
	console.error(`  FAIL: npm view ${pkg_name}@${target_version} failed:`);
	console.error(result.stderr);
	Deno.exit(1);
}

function read_sentinel(): string | null {
	try {
		return Deno.readTextFileSync(SENTINEL_PATH).trim();
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;
		return null;
	}
}

function read_cargo_version(): string {
	const match = workspace_pkg_re.exec(Deno.readTextFileSync(CARGO_PATH));
	return match?.[2] ?? '';
}

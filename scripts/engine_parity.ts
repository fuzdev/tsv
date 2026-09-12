/**
 * ENGINE-PARITY audit: the WASM engine and the native engine must format every
 * file to the same bytes.
 *
 * tsv's formatter is one Rust source compiled twice — to wasm32 for the npm
 * packages, natively for `tsv_cli` and the N-API addon — and the packages state
 * the equality as a contract ("wasm-API-parity"). Nothing graded it. The
 * fixture gate runs the native build alone; `scripts/validate_artifacts.ts`
 * smoke-tests three tiny inputs per variant, which proves each bundle *runs*,
 * not that it *agrees*. The gap that leaves is a wasm32-only divergence —
 * different float formatting, a different allocator's effect on an
 * iteration order, a 32-bit `usize` truncating an offset the native build
 * keeps — that every existing gate is blind to, and that reaches users as a
 * formatter which reformats their file differently depending on which package
 * they installed.
 *
 * What it does: copy each corpus root into two temp trees, format one with the
 * native CLI and the other with the WASM CLI, and require the two runs to agree
 * on **everything observable** — exit code, the changed-path list on stdout,
 * the diagnostics on stderr, and every resulting byte on disk. Both bins run
 * with the temp tree as cwd, so the paths they print are identical too.
 *
 * Why two trees rather than one tree twice: formatting A with native and then
 * re-running WASM over the SAME tree would only prove that native's output is a
 * WASM fixed point, which two engines with genuinely different outputs can both
 * satisfy. Comparing the two independent runs is the claim that actually holds.
 *
 * Blind spots. It compares the two CLI **drivers** as well as the two engines,
 * so a driver-only difference reads as a failure here (that is a real finding,
 * just a differently-located one; `scripts/test_napi_npm.ts` is where the
 * driver contract is pinned flag by flag). It sees only inputs the corpus
 * holds, and only the format path — `parse` wire equality is not graded here.
 * And it cannot see a divergence both engines share with each other but not
 * with prettier, which is the conformance gates' job.
 *
 * NOT in `deno task check`: it needs two built packages, and `check` builds
 * none. It runs in CI's `artifacts` job, after the step that builds both, and
 * locally after `deno task build:npm:all && deno task build:napi:packages`.
 *
 * Usage: deno task engines:audit [roots...]   (--json)
 *
 * Default roots: `tests/fixtures` and `tests/fixtures_compile` (always present;
 * the compile tree contributes generated JS, a shape hand-written code does not
 * produce) plus `../corpora/collections` (the real-code snapshot — a sibling
 * checkout, so absent is a warn-skip, not a failure, matching
 * `discovery:audit`). The fixture tree deliberately includes the
 * `input_invalid_*` family, so several hundred of the compared outcomes are
 * ERRORS: the two engines have to word a refusal alike as much as a format.
 */

import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { CORE_CRATES, WASM_CRATES, wasm_bundle_dir } from '../benches/js/lib/tsv_artifacts.ts';
import { assert_staged_fresh, type StagedCheck } from './check_staged_freshness.ts';
import { host_triple } from './napi_host.ts';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const repo_rel = (absolute: string): string => relative(ROOT, absolute);

/** The extensions `tsv format` claims — the JS/TS family, Svelte, CSS. */
const EXTENSIONS = ['.ts', '.mts', '.cts', '.js', '.mjs', '.cjs', '.svelte', '.css'];

const args = Deno.args.filter((a) => a !== '--json');
const json = Deno.args.includes('--json');

/**
 * The native CLI binary. The staged platform package's copy first — that is the
 * one that ships beside the addon, so grading it grades what a user of
 * `@fuzdev/tsv` runs — falling back to a plain `cargo build -p tsv_cli
 * --release`, which is the same binary by a shorter path.
 *
 * The host's own package is looked for ahead of the directory scan: a local
 * staging holds only the host triple, but a runner that has gathered several
 * matrix artifacts holds several, and the first `readdir` entry is then whichever
 * the filesystem lists first — a foreign one would fail to exec.
 */
function find_native_cli(): { path: string; label: string } | undefined {
	const exe = Deno.build.os === 'windows' ? 'tsv.exe' : 'tsv';
	const pkg_root = join(ROOT, 'crates/tsv_napi/pkg');
	if (existsSync(pkg_root)) {
		const staged = (triple: string) => {
			const candidate = join(pkg_root, triple, exe);
			return existsSync(candidate)
				? { path: candidate, label: `staged platform package (${triple})` }
				: undefined;
		};
		const host = staged(host_triple());
		if (host !== undefined) return host;
		for (const entry of readdirSync(pkg_root)) {
			if (entry === 'napi') continue;
			const found = staged(entry);
			if (found !== undefined) return found;
		}
	}
	const release = join(ROOT, 'target/release', exe);
	if (existsSync(release)) return { path: release, label: 'target/release' };
	return undefined;
}

const native = find_native_cli();
const wasm_cli = join(wasm_bundle_dir('all', 'npm'), 'cli.js');

// Node, not optional: the WASM side is graded on the host it ships for. The
// rest of the npm-package test family (`test:npm*`, `test:napi:npm`) takes the
// same dependency, and the CI job this runs in installs it.
try {
	const probe = new Deno.Command('node', { args: ['--version'], stdout: 'null', stderr: 'null' });
	if (!(await probe.output()).success) throw new Error('node --version failed');
} catch {
	console.error('✗ engine parity needs `node` — the host the WASM CLI ships for');
	Deno.exit(1);
}

if (!native || !existsSync(wasm_cli)) {
	const missing = [
		native ? null : 'the native tsv_cli binary (deno task build:napi:packages)',
		existsSync(wasm_cli) ? null : 'the wasm package (deno task build:npm:all)'
	].filter(Boolean);
	console.error(`✗ engine parity needs both engines built — missing: ${missing.join(', ')}`);
	Deno.exit(1);
}

// Both bins are graded artifacts, so both get the same staleness treatment the
// rest of the staged-package tooling gets: a run over a binary from before a
// formatter change is a verdict about code that no longer exists.
const freshness: StagedCheck[] = [
	{
		label: 'native tsv_cli binary',
		staged: repo_rel(native.path),
		crates: [...CORE_CRATES, 'tsv_cli', 'tsv_ignore', 'tsv_discover'],
		files: [],
		rebuild: 'deno task build:napi:packages'
	},
	{
		label: 'wasm package cli.js',
		staged: repo_rel(wasm_cli),
		crates: [...CORE_CRATES, ...WASM_CRATES],
		files: ['crates/tsv_wasm/npm/cli.js', 'scripts/patch_npm_package.ts', 'deno.json'],
		rebuild: 'deno task build:npm:all'
	}
];
await assert_staged_fresh(freshness);

/** Every file under `dir` whose extension `tsv format` claims, repo-relative. */
function collect(dir: string, base = dir, out: string[] = []): string[] {
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		// `.git` alone: the safety nets are the CLIs' business, and the copies
		// below deliberately carry no VCS directory for them to skip.
		if (entry.name === '.git') continue;
		const full = join(dir, entry.name);
		if (entry.isDirectory()) collect(full, base, out);
		else if (entry.isFile() && EXTENSIONS.some((e) => entry.name.endsWith(e))) {
			out.push(relative(base, full));
		}
	}
	return out;
}

interface RunResult {
	status: number;
	stdout: string;
	stderr: string;
}

async function run(
	bin: string[],
	cwd: string,
	args: string[] = ['format', '.']
): Promise<RunResult> {
	const command = new Deno.Command(bin[0]!, {
		args: [...bin.slice(1), ...args],
		cwd,
		stdout: 'piped',
		stderr: 'piped'
	});
	const { code, stdout, stderr } = await command.output();
	const dec = new TextDecoder();
	return { status: code, stdout: dec.decode(stdout), stderr: dec.decode(stderr) };
}

interface RootReport {
	root: string;
	files: number;
	skipped?: string;
	findings: string[];
	native?: RunResult;
	wasm?: RunResult;
}

async function grade(root: string): Promise<RootReport> {
	const absolute = resolve(ROOT, root);
	if (!existsSync(absolute)) {
		return { root, files: 0, skipped: 'not present', findings: [] };
	}
	const files = collect(absolute);
	if (files.length === 0) {
		return { root, files: 0, skipped: 'no files in tsv’s extensions', findings: [] };
	}

	// Copied OUT of the repo on purpose: `tests/fixtures` is pruned by the root
	// `.formatignore` (the fixtures are not format fixed points by policy), so a
	// run in place would discover nothing. In a bare temp tree both bins see the
	// same files under the same rules.
	const work = await mkdtemp(join(tmpdir(), 'tsv_engines_'));
	const trees = [join(work, 'native'), join(work, 'wasm')];
	try {
		for (const file of files) {
			const bytes = readFileSync(join(absolute, file));
			for (const tree of trees) {
				const dest = join(tree, file);
				mkdirSync(dirname(dest), { recursive: true });
				Deno.writeFileSync(dest, bytes);
			}
		}

		const native_run = await run([native!.path], trees[0]!);
		// `node`, not Deno: `cli.js` ships as a Node bin (`node:worker_threads`,
		// `process.execPath` respawn), and Deno's compat layer is a different host
		// than the one users run. Grading the shipped host is the whole point.
		const wasm_run = await run(['node', wasm_cli], trees[1]!);

		const findings: string[] = [];
		if (native_run.status !== wasm_run.status) {
			findings.push(`exit code: native ${native_run.status}, wasm ${wasm_run.status}`);
		}
		if (native_run.stdout !== wasm_run.stdout) {
			findings.push('the changed-path lists differ (stdout)');
		}
		if (native_run.stderr !== wasm_run.stderr) {
			findings.push('the diagnostics differ (stderr)');
		}
		// The DRIVER half over what a single `.` root never exercises: overlapping roots
		// (the canonical-path dedup), and spellings the sort must read through (`.//`,
		// `./`) — the two classes a raw component split in `cli.js` once inverted.
		const LIST_ROOTS = ['format', '--list', './/', './', '.'];
		const native_list = await run([native!.path], trees[0]!, LIST_ROOTS);
		const wasm_list = await run(['node', wasm_cli], trees[1]!, LIST_ROOTS);
		if (native_list.status !== wasm_list.status || native_list.stdout !== wasm_list.stdout) {
			findings.push('the --list over spelled, overlapping roots differs');
		}
		// The claim that matters: the bytes on disk.
		let differing = 0;
		for (const file of files) {
			const a = readFileSync(join(trees[0]!, file));
			const b = readFileSync(join(trees[1]!, file));
			if (a.equals(b)) continue;
			differing++;
			if (differing <= 10) findings.push(`bytes differ: ${file}`);
		}
		if (differing > 10) findings.push(`… and ${differing - 10} more file(s) with differing bytes`);
		return { root, files: files.length, findings, native: native_run, wasm: wasm_run };
	} finally {
		rmSync(work, { recursive: true, force: true });
	}
}

const roots = args.length
	? args
	: ['tests/fixtures', 'tests/fixtures_compile', '../corpora/collections'];
const reports: RootReport[] = [];
for (const root of roots) reports.push(await grade(root));

if (json) {
	console.log(JSON.stringify({ native: native.label, reports }, null, 2));
} else {
	console.log(`engines: native = ${native.label}, wasm = ${repo_rel(wasm_cli)}`);
	for (const report of reports) {
		if (report.skipped) {
			console.log(`  ⚠ ${report.root} — skipped (${report.skipped})`);
			continue;
		}
		if (report.findings.length === 0) {
			const summary = (report.native?.stderr ?? '').trim().split('\n').at(-1) ?? '';
			console.log(`  ✓ ${report.root}: ${report.files} files, byte-identical — ${summary}`);
		} else {
			console.log(`  ✗ ${report.root}: ${report.files} files`);
			for (const finding of report.findings) console.log(`      ${finding}`);
		}
	}
}

const graded = reports.filter((r) => !r.skipped);
const failed = graded.filter((r) => r.findings.length > 0);
if (graded.length === 0) {
	console.error('✗ no corpus root was gradable — nothing was compared');
	Deno.exit(1);
}
if (failed.length > 0) {
	console.error(
		`\n✗ the WASM and native engines disagree on ${failed.length} root(s). One Rust source ` +
			`compiled two ways must format to the same bytes; a difference here ships as a formatter ` +
			`whose output depends on which package the user installed.`
	);
	Deno.exit(1);
}
console.log(
	`✓ the two engines agree on every byte across ${graded.reduce((n, r) => n + r.files, 0)} files`
);

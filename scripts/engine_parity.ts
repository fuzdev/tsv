/**
 * ENGINE-PARITY audit: the WASM engine and the native engine must format every
 * file to the same bytes.
 *
 * tsv's formatter is one Rust source compiled twice — to wasm32 for the npm
 * packages, natively for `tsv_cli` and the N-API addon — and the packages state
 * the equality as a contract ("wasm-API-parity"). Nothing graded it. The
 * fixture gate runs the native build alone, and the package suites run a few
 * inputs per package, which proves each bundle *runs*, not that it *agrees*.
 * The gap that leaves is a wasm32-only divergence —
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
 * **The type strip** has a leg of its own, since a strip prints its output rather than
 * writing it back. Per file, over every input of the strip's extensions in
 * `tests/fixtures_strip` and `tests/fixtures`, `tsv strip <file>` is run on both bins
 * (cwd the repo root, so the paths they print agree) and the exit code, stdout and stderr
 * must match — the CLIs and the engines together. The `../corpora` snapshot is too many
 * files to spawn twice each, so it is graded in-process instead: one Node child imports
 * the native `@fuzdev/tsv` loader and the wasm package side by side and requires
 * `strip_typescript` to return deep-equal results for every file — the code, or every
 * field of a refusal or a parse error. That half also takes the JS family's files, which
 * the CLI refuses: the API reads any source as TypeScript. Absent, the snapshot is a
 * warn-skip there too.
 *
 * **The import records** ride the same in-process half, over `tests/fixtures_imports` and
 * the snapshot: `parse_imports_typescript` for the strip's engine extensions and
 * `parse_imports_svelte` for `.svelte`, deep-equal on both engines — the records, or the
 * parse error.
 *
 * Blind spots. It compares the two CLI **drivers** as well as the two engines, so a driver-only
 * difference reads as a failure here (that is a real finding, just a differently-located one;
 * `scripts/test_napi_npm.ts` is where the driver contract is pinned flag by flag). It sees only
 * inputs the corpus holds, and only the format, strip and imports paths — `parse` wire equality is
 * not graded here. And it cannot see a divergence both engines share with each other but not with
 * prettier, which is the conformance gates' job.
 *
 * NOT in `deno task check`: it needs two built packages, and `check` builds
 * none. It runs in CI's `artifacts` job, after the step that builds both, and
 * locally after `deno task build:npm:all && deno task build:napi:packages`.
 *
 * Usage: node scripts/roots/engine_parity.ts [roots...]   (--json)
 *
 * Default roots: `tests/fixtures` and `tests/fixtures_compile` (always present;
 * the compile tree contributes generated JS, a shape hand-written code does not
 * produce) plus `../corpora/collections` (the real-code snapshot — a sibling
 * checkout, so absent is a warn-skip, not a failure, matching
 * `discovery:audit`). The fixture tree deliberately includes the
 * `input_invalid_*` family, so several hundred of the compared outcomes are
 * ERRORS: the two engines have to word a refusal alike as much as a format.
 * A root resolves against the checkout. Every failure exits 1.
 *
 * Reach: the checkout read-only; each root read-only; temp directories, for the two trees and
 * the in-process half's consumer project; `node`, and the native CLI by its path beneath the
 * checkout (`EngineParityDeps`, built by `scripts/roots/engine_parity.ts`).
 *
 * @module
 */

import { dirname, extname, join, relative, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { CORE_CRATES } from '../benches/js/lib/tsv_artifacts.ts';
import {
	NAPI_LOADER_DIR,
	NAPI_PKG_ROOT,
	NAPI_STAGING_SOURCES,
	napi_addon_check,
	napi_loader_checks,
	napi_staged_triple_of,
	type StagedCheck,
	staged_freshness_report,
	wasm_package_checks,
	wasm_package_dir
} from './check_staged_freshness.ts';
import { signal_exit_code } from './exit_code.ts';
import { copy_between } from './handle_copy.ts';
import { handle_source_tree } from './handle_source_tree.ts';
import { cli_binary_name } from './napi_host.ts';

/** What `run_engine_parity` reaches. */
export interface EngineParityDeps {
	/** The checkout: the staged packages, the native binary, and the sources dating them. */
	checkout: FsReadDir;
	/** Opens a corpus root, resolved against the checkout: `null` when nothing is there. */
	open_root: (root: string) => Promise<FsReadDir | null>;
	/** Makes a fresh directory removed when the leg that made it ends. */
	temp_dir: () => Promise<FsDir & AsyncDisposable>;
	/** The host's platform triple, whose staged package is looked for first. */
	host_triple: string;
	/** The native CLI at `path`, checkout-relative, pinned. */
	native_program: (path: string) => SpawnProgram;
	/** `node`, the host the WASM CLI ships for: `null` when it isn't on `PATH`. */
	node: SpawnProgram | null;
	/** `BENCH_STALE_OK=1`: a stale artifact warns instead of failing the run. */
	stale_ok: boolean;
	print: (line: string) => void;
	print_error: (line: string) => void;
}

/** The names of the entries of the directory at `path` beneath `dir`, sorted by code unit, the
 * order `node:fs`'s listing gives (libuv sorts it), so what this prints keeps one order. */
async function list_names(dir: FsReadDir, path?: string): Promise<Array<string>> {
	const names: Array<string> = [];
	for await (const entry of dir.list(path)) names.push(entry.name);
	return names.sort();
}

/** The WASM CLI, the `all` package's bin, checkout-relative. */
const WASM_CLI = `${wasm_package_dir('all')}/cli.js`;

/** The native CLI binary to grade (`find_native_cli`). */
export interface NativeCli {
	/** Checkout-relative. */
	path: string;
	label: string;
}

/**
 * The native CLI binary. The staged platform package's copy first — that is the
 * one that ships beside the addon, so grading it grades what a user of
 * `@fuzdev/tsv` runs — falling back to a plain `cargo build -p tsv_cli
 * --release`, which is the same binary by a shorter path.
 *
 * The host's own package is looked for ahead of the directory scan: a local
 * staging holds only the host triple, but a runner that has gathered several
 * matrix artifacts holds several, and the first listed entry is then whichever
 * the filesystem lists first — a foreign one would fail to exec.
 */
export async function find_native_cli(
	checkout: FsReadDir,
	host_triple: string
): Promise<NativeCli | undefined> {
	const exe = cli_binary_name(host_triple);
	if (await checkout.exists(NAPI_PKG_ROOT)) {
		const staged = async (triple: string): Promise<NativeCli | undefined> => {
			const candidate = `${NAPI_PKG_ROOT}/${triple}/${exe}`;
			return (await checkout.exists(candidate))
				? { path: candidate, label: `staged platform package (${triple})` }
				: undefined;
		};
		const host = await staged(host_triple);
		if (host !== undefined) return host;
		for (const name of await list_names(checkout, NAPI_PKG_ROOT)) {
			if (name === 'napi') continue;
			const found = await staged(name);
			if (found !== undefined) return found;
		}
	}
	const release = `target/release/${exe}`;
	if (await checkout.exists(release)) return { path: release, label: 'target/release' };
	return undefined;
}

/** One child's outcome: its exit code (`128 + N` for signal `N`) and its output. */
export interface RunResult {
	status: number;
	stdout: string;
	stderr: string;
}

/** Run `program` in `cwd` and capture its outcome. */
async function run(
	program: SpawnProgram,
	cwd: FsReadDir,
	args: ReadonlyArray<string>
): Promise<RunResult> {
	const result = await program.out(args, { cwd });
	if (result.truncated) {
		throw new Error(`${program.name} ${args.join(' ')} wrote more output than is captured`);
	}
	let status: number;
	if (result.kind === 'exited') status = result.code;
	else if (result.kind === 'signaled') status = signal_exit_code(result.signal);
	else throw new Error(`${program.name} ${args.join(' ')} was stopped (${result.kind})`);
	return { status, stdout: result.stdout, stderr: result.stderr };
}

interface RootReport {
	root: string;
	files: number;
	skipped?: string;
	findings: string[];
	native?: RunResult;
	wasm?: RunResult;
}

/** Whether two byte arrays hold the same bytes. */
const same_bytes = (a: Uint8Array, b: Uint8Array): boolean =>
	a.length === b.length && a.every((byte, i) => byte === b[i]);

/** The extensions `tsv strip` takes (`tsv_discover::STRIPPABLE_EXTENSIONS`). */
const STRIP_EXTENSIONS = ['.ts', '.mts', '.cts'];

/** The extensions the `strip_typescript` engine half grades: the CLI's, plus the JS family —
 * the API reads any source as TypeScript, so a JS file is still an input it takes. */
const STRIP_ENGINE_EXTENSIONS = [...STRIP_EXTENSIONS, '.js', '.mjs', '.cjs'];

/**
 * Every regular file beneath `dir` whose extension is in `extensions`, relative to it, the
 * extension read as both CLIs read it, `Path::extension` without regard to ASCII case
 * (`extname` lowercased: a bare dotfile has none, `A.TS` is a TypeScript file). A symlink is
 * neither descended into nor taken, and `skip` names the entries passed over. Each directory's
 * entries are taken in `list_names`' order, so the findings name the files in one order.
 */
async function collect(
	dir: FsReadDir,
	extensions: ReadonlyArray<string>,
	skip: ReadonlyArray<string>,
	path = '',
	out: string[] = []
): Promise<string[]> {
	const entries = [];
	for await (const entry of dir.list(path === '' ? undefined : path)) entries.push(entry);
	entries.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
	for (const entry of entries) {
		if (skip.includes(entry.name)) continue;
		const rel = path === '' ? entry.name : `${path}/${entry.name}`;
		if (entry.kind === 'directory') await collect(dir, extensions, skip, rel, out);
		else if (entry.kind === 'file' && extensions.includes(extname(entry.name).toLowerCase())) {
			out.push(rel);
		}
	}
	return out;
}

/** The in-process half: one Node child holding both engines, one export compared per file
 * (results as JSON, deep-equal by their serialization — every field is a string, a number,
 * a boolean, `null` or a plain object or array of them, in one key order on both engines).
 * The list pairs each file with the export that reads it. */
const ENGINE_PARITY_SCRIPT = `
import { readFileSync } from 'node:fs';
import * as native from '@fuzdev/tsv';
const wasm = await import(process.argv[2]);
const files = JSON.parse(readFileSync(process.argv[3], 'utf8'));
const differing = [];
for (const [file, name] of files) {
	const source = readFileSync(file, 'utf8');
	const a = JSON.stringify(native[name](source));
	const b = JSON.stringify(wasm[name](source));
	if (a !== b) differing.push(file);
}
process.stdout.write(JSON.stringify({ files: files.length, differing }));
`;

/**
 * The audit (module doc).
 * @param argv - the roots to grade, none for the defaults, and `--json` for the reports as JSON
 * @returns the exit code: `0` when the engines agree on every graded root, `1` otherwise
 */
export async function run_engine_parity(
	deps: EngineParityDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	const { checkout, print, print_error } = deps;
	const args = argv.filter((a) => a !== '--json');
	const json = argv.includes('--json');

	const native = await find_native_cli(checkout, deps.host_triple);

	// Node, not optional: the WASM side is graded on the host it ships for. The
	// rest of the npm-package test family (`test:npm*`, `test:napi:npm`) takes the
	// same dependency, and the CI job this runs in installs it.
	const node_ok =
		deps.node !== null &&
		(await deps.node.out(['--version']).then(
			(result) => result.kind === 'exited' && result.code === 0,
			() => false
		));
	if (!node_ok) {
		print_error('✗ engine parity needs `node` — the host the WASM CLI ships for');
		return 1;
	}
	const node = deps.node!;

	const wasm_built = await checkout.exists(WASM_CLI);
	if (!native || !wasm_built) {
		const missing = [
			native ? null : 'the native tsv_cli binary (deno task build:napi:packages)',
			wasm_built ? null : 'the wasm package (deno task build:npm:all)'
		].filter(Boolean);
		print_error(`✗ engine parity needs both engines built — missing: ${missing.join(', ')}`);
		return 1;
	}
	const native_bin = deps.native_program(native.path);
	const wasm_cli = join(checkout.path, WASM_CLI);

	// Both bins are graded artifacts, so both get the same staleness treatment the
	// rest of the staged-package tooling gets: a run over a binary from before a
	// formatter change is a verdict about code that no longer exists.
	const freshness: StagedCheck[] = [
		{
			label: 'native tsv_cli binary',
			staged: native.path,
			crates: [...CORE_CRATES, 'tsv_cli', 'tsv_ignore', 'tsv_discover'],
			// the N-API staging copies it into the platform package, so an edit to the staging's
			// sources re-stages a STAGED binary (as `scripts/test_napi_npm.ts` names for the same
			// artifact) — a `target/release` fallback it never touched is not aged by it
			files: native.label === 'target/release' ? [] : NAPI_STAGING_SOURCES,
			rebuild: 'deno task build:napi:packages'
		},
		// the wasm package as every reader of its staging dates it — the engine bytes
		// themselves included, since `cli.js` is re-copied on every patcher run and a fresh
		// `cli.js` over a stale `.wasm` would otherwise grade as fresh
		...wasm_package_checks('all')
	];
	// the strip leg's in-process half imports the staged native loader and its addon
	const loader_staged = await checkout.exists(NAPI_LOADER_DIR);
	const staged = napi_staged_triple_of(
		loader_staged,
		loader_staged ? await list_names(checkout, NAPI_PKG_ROOT) : []
	);
	if (!staged.ok) {
		print_error(staged.message);
		return 1;
	}
	const napi_triple = staged.triple;
	freshness.push(...napi_loader_checks(), napi_addon_check(napi_triple));
	const report = await staged_freshness_report(freshness, {
		tree: handle_source_tree(checkout),
		stale_ok: deps.stale_ok
	});
	if (report !== undefined) {
		print_error(report.message);
		if (report.fatal) return 1;
	}

	/**
	 * The extensions `tsv format` claims, read off the native binary rather than kept by
	 * hand: a `format` over an empty directory refuses with the set rendered from
	 * `tsv_discover::FORMATTABLE_EXTENSIONS` (`No files to format — no unignored
	 * .ts/.mts/… files in scope`), so a ninth language reaches this script's file
	 * selection without anyone editing it.
	 */
	let extensions: string[];
	{
		await using empty = await deps.temp_dir();
		const refusal = await run(native_bin, empty, ['format', '.']);
		const listed = /no unignored (\S+) files in scope/.exec(refusal.stderr)?.[1];
		if (refusal.status !== 2 || listed === undefined) {
			print_error(
				`✗ could not read the extension set off the native binary's empty-scope refusal:\n${refusal.stderr}`
			);
			return 1;
		}
		extensions = listed.split('/');
	}

	/** The format leg over one root. */
	const grade = async (root: string): Promise<RootReport> => {
		const dir = await deps.open_root(root);
		if (dir === null) return { root, files: 0, skipped: 'not present', findings: [] };
		// `.git` alone: the safety nets are the CLIs' business, and the copies
		// below deliberately carry no VCS directory for them to skip.
		const files = await collect(dir, extensions, ['.git']);
		if (files.length === 0) {
			return { root, files: 0, skipped: 'no files in tsv’s extensions', findings: [] };
		}

		// Copied OUT of the repo on purpose: `tests/fixtures` is pruned by the root
		// `.formatignore` (the fixtures are not format fixed points by policy), so a
		// run in place would discover nothing. In a bare temp tree both bins see the
		// same files under the same rules.
		await using work = await deps.temp_dir();
		await work.mkdir('native');
		await work.mkdir('wasm');
		const trees = [work.dir('native'), work.dir('wasm')] as const;
		for (const file of files) {
			const bytes = await dir.read(file);
			for (const tree of trees) {
				const parent = dirname(file);
				if (parent !== '.') await tree.mkdir(parent, { recursive: true });
				await tree.write(file, bytes);
			}
		}

		const native_run = await run(native_bin, trees[0], ['format', '.']);
		// `node`, not Deno: `cli.js` ships as a Node bin (`node:worker_threads`,
		// `process.execPath` respawn), and Deno's compat layer is a different host
		// than the one users run. Grading the shipped host is the whole point.
		const wasm_run = await run(node, trees[1], [wasm_cli, 'format', '.']);

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
		const native_list = await run(native_bin, trees[0], LIST_ROOTS);
		const wasm_list = await run(node, trees[1], [wasm_cli, ...LIST_ROOTS]);
		if (native_list.status !== wasm_list.status || native_list.stdout !== wasm_list.stdout) {
			findings.push('the --list over spelled, overlapping roots differs');
		}
		// The claim that matters: the bytes on disk.
		let differing = 0;
		for (const file of files) {
			const a = await trees[0].read(file);
			const b = await trees[1].read(file);
			if (same_bytes(a, b)) continue;
			differing++;
			if (differing <= 10) findings.push(`bytes differ: ${file}`);
		}
		if (differing > 10) findings.push(`… and ${differing - 10} more file(s) with differing bytes`);
		return { root, files: files.length, findings, native: native_run, wasm: wasm_run };
	};

	/** A file beneath `root` as a path from the checkout, the form the strip CLI takes it in. */
	const from_checkout = (root: string, file: string): string =>
		relative(checkout.path, resolve(checkout.path, root, file));

	/** The strip leg over the CLIs: `tsv strip <file>` on both bins, per file, a few at a time. */
	const grade_strip_cli = async (root: string): Promise<RootReport> => {
		const dir = await deps.open_root(root);
		if (dir === null) return { root, files: 0, skipped: 'not present', findings: [] };
		const files = (await collect(dir, STRIP_EXTENSIONS, ['.git', 'node_modules']))
			.map((file) => from_checkout(root, file))
			.sort();
		if (files.length === 0) {
			return { root, files: 0, skipped: 'no files in the strip’s extensions', findings: [] };
		}
		const findings: string[] = [];
		let differing = 0;
		let next = 0;
		const worker = async (): Promise<void> => {
			while (next < files.length) {
				const file = files[next++]!;
				const [native_run, wasm_run] = await Promise.all([
					run(native_bin, checkout, ['strip', file]),
					run(node, checkout, [wasm_cli, 'strip', file])
				]);
				if (
					native_run.status === wasm_run.status &&
					native_run.stdout === wasm_run.stdout &&
					native_run.stderr === wasm_run.stderr
				) {
					continue;
				}
				differing++;
				if (differing <= 10) {
					findings.push(
						`tsv strip ${file}: exit native ${native_run.status} / wasm ${wasm_run.status}` +
							(native_run.stdout === wasm_run.stdout ? '' : ', stdout differs') +
							(native_run.stderr === wasm_run.stderr ? '' : ', stderr differs')
					);
				}
			}
		};
		await Promise.all(Array.from({ length: 4 }, worker));
		if (differing > 10) findings.push(`… and ${differing - 10} more file(s) whose strip differs`);
		return { root: `${root} (tsv strip)`, files: files.length, findings };
	};

	/**
	 * An export graded over the engines alone, in process (see the module doc): `label` names
	 * the leg, `extensions` the files it takes, and `export_for` the export reading each one.
	 */
	const grade_engines = async (
		root: string,
		label: string,
		extensions: string[],
		export_for: (file: string) => string
	): Promise<RootReport> => {
		const dir = await deps.open_root(root);
		if (dir === null) {
			return { root: `${root} (${label})`, files: 0, skipped: 'not present', findings: [] };
		}
		// the child reads each file by its absolute path
		const files = (await collect(dir, extensions, ['.git', 'node_modules'])).map((file) =>
			resolve(checkout.path, root, file)
		);
		if (files.length === 0) {
			return {
				root: `${root} (${label})`,
				files: 0,
				skipped: `no files in ${label}’s extensions`,
				findings: []
			};
		}
		await using consumer = await deps.temp_dir();
		// the staged loader and its platform package, laid into the consumer's `node_modules`
		// as npm installs them, so the child imports the loader by its bare name
		await copy_between(checkout, NAPI_LOADER_DIR, consumer, 'node_modules/@fuzdev/tsv');
		await copy_between(
			checkout,
			`${NAPI_PKG_ROOT}/${napi_triple}`,
			consumer,
			`node_modules/@fuzdev/tsv-${napi_triple}`
		);
		await consumer.write('engine_parity.mjs', ENGINE_PARITY_SCRIPT);
		await consumer.write(
			'files.json',
			JSON.stringify(files.map((file) => [file, export_for(file)]))
		);
		const wasm_index = pathToFileURL(join(checkout.path, wasm_package_dir('all'), 'index.js')).href;
		const result = await run(node, consumer, [
			join(consumer.path, 'engine_parity.mjs'),
			wasm_index,
			join(consumer.path, 'files.json')
		]);
		if (result.status !== 0) {
			return {
				root: `${root} (${label})`,
				files: files.length,
				findings: [`the comparison child failed (exit ${result.status}): ${result.stderr.trim()}`]
			};
		}
		const { files: compared, differing } = JSON.parse(result.stdout) as {
			files: number;
			differing: string[];
		};
		const findings = differing.slice(0, 10).map((file) => `${label} differs: ${file}`);
		if (differing.length > 10) findings.push(`… and ${differing.length - 10} more file(s)`);
		if (compared !== files.length) {
			findings.push(`compared ${compared} of ${files.length} files`);
		}
		return { root: `${root} (${label})`, files: files.length, findings };
	};

	/** The strip leg over the engines alone. */
	const grade_strip_engines = (root: string): Promise<RootReport> =>
		grade_engines(root, 'strip_typescript', STRIP_ENGINE_EXTENSIONS, () => 'strip_typescript');

	/** The import-records leg over the engines alone: TypeScript's export for the strip's engine
	 * extensions, Svelte's for a component. */
	const grade_imports_engines = (root: string): Promise<RootReport> =>
		grade_engines(root, 'parse_imports_*', [...STRIP_ENGINE_EXTENSIONS, '.svelte'], (file) =>
			extname(file).toLowerCase() === '.svelte'
				? 'parse_imports_svelte'
				: 'parse_imports_typescript'
		);

	const roots = args.length
		? args
		: ['tests/fixtures', 'tests/fixtures_compile', '../corpora/collections'];
	const reports: RootReport[] = [];
	for (const root of roots) reports.push(await grade(root));
	// the strip leg: its CLI half over the fixture trees, its engine half over the snapshot
	// (a run naming its own roots grades them through both halves)
	const strip_cli_roots = args.length ? args : ['tests/fixtures_strip', 'tests/fixtures'];
	const strip_engine_roots = args.length ? args : ['../corpora/collections'];
	for (const root of strip_cli_roots) reports.push(await grade_strip_cli(root));
	for (const root of strip_engine_roots) reports.push(await grade_strip_engines(root));
	// the import records: the fixture tree and the snapshot, in process
	const imports_engine_roots = args.length
		? args
		: ['tests/fixtures_imports', '../corpora/collections'];
	for (const root of imports_engine_roots) reports.push(await grade_imports_engines(root));

	if (json) {
		print(JSON.stringify({ native: native.label, reports }, null, 2));
	} else {
		print(`engines: native = ${native.label}, wasm = ${WASM_CLI}`);
		for (const report of reports) {
			if (report.skipped) {
				print(`  ⚠ ${report.root} — skipped (${report.skipped})`);
				continue;
			}
			if (report.findings.length === 0) {
				const summary = (report.native?.stderr ?? '').trim().split('\n').at(-1) ?? '';
				print(
					`  ✓ ${report.root}: ${report.files} files, byte-identical${summary ? ` — ${summary}` : ''}`
				);
			} else {
				print(`  ✗ ${report.root}: ${report.files} files`);
				for (const finding of report.findings) print(`      ${finding}`);
			}
		}
	}

	const graded = reports.filter((r) => !r.skipped);
	const failed = graded.filter((r) => r.findings.length > 0);
	if (graded.length === 0) {
		print_error('✗ no corpus root was gradable — nothing was compared');
		return 1;
	}
	if (failed.length > 0) {
		print_error(
			`\n✗ the WASM and native engines disagree on ${failed.length} root(s). One Rust source ` +
				`compiled two ways must format to the same bytes; a difference here ships as a formatter ` +
				`whose output depends on which package the user installed.`
		);
		return 1;
	}
	print(
		`✓ the two engines agree on every byte across ${graded.reduce((n, r) => n + r.files, 0)} files`
	);
	return 0;
}

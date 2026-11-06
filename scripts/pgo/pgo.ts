/**
 * The profile-guided build of the two shipped native binaries — the `tsv_cli`
 * binary (`release`) and the N-API addon (`napi` profile) — and the gates that
 * make a wrong one fail rather than ship. One subcommand per step, so the release
 * workflow can run each in the environment its row needs (a container for the
 * cargo builds and the musl training, the host for staging and the merge), and
 * `all`, which runs them in order — `deno task build:pgo`, the local pipeline.
 *
 * The steps, in order (`all`):
 *
 * 1. `corpora` — the training corpus is the `../corpora` snapshot's collections of
 *    manifest `origin: own`, as authored. The checkout must be at the pinned
 *    `collections/` tree (`GATE_CHECKOUT_IDS`), clean, with every own collection
 *    present; the manifest and the snapshot's `.gitignore` are read from the pinned
 *    COMMIT, not the working tree, and written to `corpus.json` for the steps that run
 *    without git. `--fetch` shallow-fetches the pinned commit first (the release rows).
 * 2. the plain `release` CLI — the byte-identity reference (`target/`, as ever).
 * 3. `build.sh instrument` — both binaries with `-Cprofile-generate`.
 * 4. `stage --phase instrument` — the instrumented pair as an installed loader +
 *    platform package, so the addon trains through the loader's own `cli.js`.
 * 5. `train` — each binary AS ITSELF over its own fresh corpus copy, every shipped
 *    surface: `parse` on every file (the wire-JSON path, which a format never takes;
 *    the CLI also with `--locations`, its `loc` writer), the type strip on every
 *    TypeScript file, the import records on every TypeScript and Svelte file (the
 *    addon's alone: the CLI has none), then `format --jobs 1` over the tree; the CLI
 *    directly, the addon through `cli.js` and, for the strip and the import records, in
 *    process (`addon_ops.ts`). `--jobs 1` keeps WHAT runs fixed — the same calls in the
 *    same order — but not the profile's bytes: hash tables keyed on addresses or seeded
 *    per process count differently run to run, so two trainings write different
 *    profiles and the binaries' bytes differ. That is the contract, not a gap:
 *    what is gated is the OUTPUT (step 11). Every file must parse, every strip must answer
 *    or refuse, and the format must exit 0, or the step fails.
 * 6. `merge` — `llvm-profdata merge` per world, with the pinned toolchain's own
 *    `llvm-profdata` (`llvm-tools-preview`; its LLVM must be the one the instrumenting
 *    rustc reported, which a container row ran), then the NON-EMPTY gate: the profile
 *    counts function entries (`build.sh instrument` passes `-pgo-instrument-entry`), and
 *    every hot function (`HOT_FUNCTIONS`) was entered once for every input of its class
 *    it trained on (`floor_of`).
 * 7. `build.sh use` — both binaries with `-Cprofile-use` and the cost-benefit
 *    inliner off.
 * 8. `gate-builds` — the profile reached every workspace crate's compile (the
 *    `-Cprofile-use` flag on its rustc command line), and no profile diagnostic in
 *    the use builds' logs but the build scripts' (which never run in training): a
 *    hash mismatch or a missing function means a stale or partly applied profile,
 *    which falls back to no PGO silently.
 * 9. `gate-sections` — the instrumented binaries carry `__llvm_prf` sections and
 *    the use binaries none (the check's own positive control first).
 * 10. `stage --phase use` — the PGO pair into `crates/tsv_napi/pkg/<triple>/`, the
 *     platform package that ships.
 * 11. `byteid` — PGO changes code layout, never output: the PGO CLI and the PGO addon
 *     (through `cli.js`) must format the whole snapshot and every fixture to the plain
 *     CLI's bytes (trees, exit codes, output); over the fixture inputs plus a stride of
 *     the snapshot, the PGO CLI must parse (with and without `--locations`) and strip to
 *     the plain CLI's bytes, and the PGO addon parse and strip to them in process; the
 *     PGO addon's import records must equal the instrumented addon's (the same source
 *     and optimization without the profile — the CLI has no import records).
 * 12. the size bands (`validate_napi_artifact.ts --pgo`), so a silently re-enabled
 *     inliner reads as a size jump.
 * 13. `depth-shapes` + `depth` — the depth limits' stack contract in the staged PGO
 *     addon itself (`addon_depth.ts`), whose frames no test build reproduces. On Windows
 *     each pass runs under its own scratch node.exe copy whose PE stack reserve sits just
 *     above the pass's request, so a worker's requested stack sizes its thread at the
 *     nearly the tenth margin of elsewhere; where that still cannot grade (a copy does not
 *     start, the gate's thread would exceed the contract, or the control
 *     survives on a thread the reserve sized), the plain-profile native gate
 *     (`test:depth:napi`'s run) grades it instead (`cmd_depth`).
 *     `--no-depth` skips it locally; the release rows always run it.
 *
 * Runs under Deno (`deno task build:pgo`) and under Node ≥ 22.18 (the release rows,
 * and inside node:alpine for the musl row's training, byte-identity and depth), so it
 * uses node builtins only. Run from the repo root.
 *
 * Options: `--dir` the working directory (default `.pgo`), `--triple` the platform
 * triple (default the host's), `--target` a cross row's rust target, `--corpora` the
 * snapshot checkout (default `../corpora`), `--reference` the plain CLI `byteid`
 * compares against (default the `release` build under `target/`), `--shapes` a shapes
 * file for `depth` (default the one `depth-shapes` writes under `--dir`).
 *
 * Usage: deno task build:pgo [--dir <d>] [--triple <t>] [--target <rust-target>]
 *          [--corpora <root>] [--no-depth]
 *        node scripts/pgo/pgo.ts <corpora [--fetch]|stage --phase <p>|train|merge|
 *          gate-builds|gate-sections|byteid|depth-shapes|depth|all> [options]
 *
 * @module
 */

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	cpSync,
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	rmSync,
	statSync,
	writeFileSync
} from 'node:fs';
import { join, relative, resolve } from 'node:path';
import process from 'node:process';
import { parseArgs } from 'node:util';

import {
	CORPORA_MANIFEST_VERSION,
	CORPORA_ROOT,
	CORPORA_TREE,
	CORPORA_URL
} from '../../benches/js/lib/corpora.ts';
import { GATE_CHECKOUT_IDS } from '../../benches/js/lib/gate_counts.ts';
import { write_shape_limits } from '../depth_shapes.ts';
import { type AddonRecord, is_strip_path } from './addon_ops.ts';
import {
	CANNOT_GRADE_EXIT,
	CANNOT_GRADE_LINE,
	CONTROL_STACK_RESERVE,
	GATE_STACK_RESERVE,
	pe_set_stack_reserve,
	pe_stack_sizes,
	spawn_did_not_start
} from './depth_stack.ts';
import {
	cargo_artifacts,
	loader_entries,
	parse_language,
	pgo_paths,
	type Phase,
	WORLDS
} from './layout.ts';
import {
	grade_profile,
	has_instrumentation_sections,
	llvm_version,
	parse_pgo_diagnostics,
	profile_use_crates,
	type TrainedCounts
} from './profile.ts';

/** Every Nth file of the snapshot joins the per-file half of the byte-identity gate (the
 * format half takes them all; the CLI's parse and strip spawn a process per file per arm). */
const PARSE_SAMPLE_STRIDE = 8;

/** The workspace crates both shipped binaries compile, each of whose rustc invocations
 * must carry the world's `-Cprofile-use` (`gate-builds`). */
const PROFILED_CRATES = ['tsv_lang', 'tsv_ts', 'tsv_css', 'tsv_svelte'] as const;

const { values: args, positionals } = parseArgs({
	allowPositionals: true,
	options: {
		dir: { type: 'string', default: '.pgo' },
		triple: { type: 'string' },
		target: { type: 'string' },
		corpora: { type: 'string', default: CORPORA_ROOT },
		phase: { type: 'string' },
		reference: { type: 'string' },
		shapes: { type: 'string' },
		fetch: { type: 'boolean', default: false },
		'no-depth': { type: 'boolean', default: false }
	}
});

class PipelineError extends Error {}
const fail = (message: string): never => {
	throw new PipelineError(message);
};

/** The node binary: this process under Node, `node` from PATH under Deno. */
const NODE = typeof Deno !== 'undefined' ? 'node' : process.execPath;
const DENO = typeof Deno !== 'undefined' ? Deno.execPath() : 'deno';

/** The host's platform triple, in the platform packages' terms. `scripts/napi_host.ts`
 * spells the same from `Deno.build`, which this pipeline cannot read under Node. */
const host_triple = (): string => {
	const cpu = process.arch;
	if (process.platform === 'win32') return `win32-${cpu}`;
	if (process.platform === 'darwin') return `darwin-${cpu}`;
	if (process.platform === 'linux') {
		const report = process.report?.getReport() as { header?: { glibcVersionRuntime?: string } };
		return `linux-${cpu}-${report?.header?.glibcVersionRuntime ? 'gnu' : 'musl'}`;
	}
	return fail(`no platform triple for ${process.platform}-${cpu} — pass --triple`);
};

const dir = resolve(args.dir!);
const triple = args.triple ?? host_triple();
const rust_target = args.target || undefined;
const corpora_root = resolve(args.corpora!);
const paths = pgo_paths(dir);
const artifacts = (phase: Phase) => cargo_artifacts(paths.target(phase), triple, rust_target);

interface RunResult {
	status: number;
	stdout: string;
	stderr: string;
}

/** Run a command to completion, capturing its output. A spawn failure (no such binary)
 * is a pipeline failure, never a nonzero status to grade. */
const capture = (cmd: string, cmd_args: string[], env?: Record<string, string>): RunResult => {
	const r = spawnSync(cmd, cmd_args, {
		encoding: 'utf8',
		maxBuffer: 1 << 30,
		env: env ? { ...process.env, ...env } : process.env
	});
	if (r.error) fail(`could not run ${cmd}: ${r.error.message}`);
	return { status: r.status ?? -1, stdout: r.stdout, stderr: r.stderr };
};

/** Run a command with inherited stdio; a nonzero exit fails the pipeline. */
const run = (cmd: string, cmd_args: string[], env?: Record<string, string>): void => {
	console.log(`$ ${cmd} ${cmd_args.join(' ')}`);
	const r = spawnSync(cmd, cmd_args, {
		stdio: 'inherit',
		env: env ? { ...process.env, ...env } : process.env
	});
	if (r.error) fail(`could not run ${cmd}: ${r.error.message}`);
	if (r.status !== 0) fail(`${cmd} ${cmd_args[0] ?? ''} exited ${r.status ?? r.signal}`);
};

const git = (...git_args: string[]): string => {
	const r = capture('git', ['-C', corpora_root, ...git_args]);
	if (r.status !== 0)
		fail(`git ${git_args.join(' ')} in ${corpora_root} failed: ${r.stderr.trim()}`);
	return r.stdout.trim();
};

const sha256_file = (path: string): string =>
	createHash('sha256').update(readFileSync(path)).digest('hex');

const require_file = (path: string, remedy: string): string => {
	if (!existsSync(path) || statSync(path).size === 0)
		fail(`${path} is missing or empty — ${remedy}`);
	return path;
};

// --- 1. the training corpus -------------------------------------------------------

/** The training corpus `corpora` resolved from the pinned commit (`pgo_paths().corpus`). */
interface Corpus {
	/** The `origin: own` collections, in manifest order. */
	own: string[];
	/** The snapshot's root `.gitignore` at the pinned commit, or `null` when it has none. */
	gitignore: string | null;
}

/** The pinned commit's `origin: own` collections, each required present and non-empty
 * in the checkout. The manifest is read from the COMMIT, so a working-tree edit or a
 * HEAD that agrees only on `collections/` cannot move the training set. */
const own_collections = (commit: string): string[] => {
	const shown = capture('git', ['-C', corpora_root, 'show', `${commit}:manifest.json`]);
	if (shown.status !== 0) fail(`${commit} holds no manifest.json: ${shown.stderr.trim()}`);
	const manifest = JSON.parse(shown.stdout) as {
		version?: unknown;
		collections?: { name?: unknown; origin?: unknown }[];
	};
	if (manifest.version !== CORPORA_MANIFEST_VERSION || !Array.isArray(manifest.collections)) {
		fail(`${commit}:manifest.json is not a version-${CORPORA_MANIFEST_VERSION} manifest`);
	}
	const own = manifest.collections!.filter((c) => c.origin === 'own').map((c) => String(c.name));
	if (own.length === 0) fail(`${commit}:manifest.json names no collection of origin \`own\``);
	const missing = own.filter((name) => {
		const path = join(corpora_root, CORPORA_TREE, name);
		return !existsSync(path) || readdirSync(path).length === 0;
	});
	if (missing.length > 0) {
		fail(`own collections missing from ${join(corpora_root, CORPORA_TREE)}: ${missing.join(', ')}`);
	}
	return own;
};

const cmd_corpora = (fetch: boolean): void => {
	const pin = GATE_CHECKOUT_IDS[CORPORA_ROOT];
	if (!pin?.commit) fail(`GATE_CHECKOUT_IDS['${CORPORA_ROOT}'] records no commit to fetch`);
	const commit = pin!.commit!;
	if (fetch && !existsSync(join(corpora_root, '.git'))) {
		mkdirSync(corpora_root, { recursive: true });
		git('init', '-q');
		git('fetch', '--depth', '1', '--no-tags', CORPORA_URL, commit);
		git('checkout', '-q', '--detach', 'FETCH_HEAD');
	}
	if (!existsSync(join(corpora_root, '.git'))) {
		fail(`${corpora_root} is not a git checkout — pass --fetch, or clone ${CORPORA_URL} there`);
	}
	const head_tree = git('rev-parse', `HEAD:${CORPORA_TREE}`);
	if (!head_tree.startsWith(pin!.hash)) {
		fail(`${corpora_root} HEAD:${CORPORA_TREE} is ${head_tree}, not the pinned ${pin!.hash}`);
	}
	const commit_tree = capture('git', [
		'-C',
		corpora_root,
		'rev-parse',
		`${commit}:${CORPORA_TREE}`
	]);
	if (commit_tree.status !== 0) {
		fail(
			`the pinned commit ${commit} is not in ${corpora_root} (git -C ${corpora_root} fetch origin ${commit})`
		);
	}
	if (commit_tree.stdout.trim() !== head_tree) {
		fail(
			`the pinned commit ${commit}'s ${CORPORA_TREE} is ${commit_tree.stdout.trim()}, not the pinned ` +
				`tree ${head_tree} — re-record GATE_CHECKOUT_IDS' commit with its tree`
		);
	}
	const dirty = git('status', '--porcelain', '--untracked-files=all', '--', CORPORA_TREE);
	if (dirty !== '') fail(`${corpora_root}/${CORPORA_TREE} has local changes:\n${dirty}`);
	const own = own_collections(commit);
	const gitignore = capture('git', ['-C', corpora_root, 'show', `${commit}:.gitignore`]);
	const corpus: Corpus = { own, gitignore: gitignore.status === 0 ? gitignore.stdout : null };
	mkdirSync(dir, { recursive: true });
	writeFileSync(paths.corpus, JSON.stringify(corpus, null, '\t') + '\n');
	console.log(
		`corpora: ${CORPORA_TREE} at ${head_tree.slice(0, 12)}; own collections: ${own.join(', ')}`
	);
};

/** The corpus `corpora` resolved — required, so no step reads an unpinned working tree. */
const read_corpus = (): Corpus =>
	JSON.parse(readFileSync(require_file(paths.corpus, 'run `pgo.ts corpora` first'), 'utf8'));

// --- 4 / 10. staging -----------------------------------------------------------------

const PKG_ROOT = 'crates/tsv_napi/pkg';

const cmd_stage = (phase: Phase): void => {
	const a = artifacts(phase);
	require_file(a.cli, `run build.sh ${phase} first`);
	require_file(a.addon, `run build.sh ${phase} first`);
	run(DENO, [
		'run',
		'--allow-read',
		`--allow-write=${PKG_ROOT}`,
		'scripts/build_napi_packages.ts',
		'--triple',
		triple,
		'--artifact',
		a.addon,
		'--cli-artifact',
		a.cli
	]);
	const node_modules = paths.loader(phase);
	rmSync(node_modules, { recursive: true, force: true });
	mkdirSync(join(node_modules, '@fuzdev'), { recursive: true });
	cpSync(join(PKG_ROOT, 'napi'), join(node_modules, '@fuzdev', 'tsv'), { recursive: true });
	cpSync(join(PKG_ROOT, triple), join(node_modules, '@fuzdev', `tsv-${triple}`), {
		recursive: true
	});
	// the instrumented pair is a training tool, never a staged package
	if (phase === 'instrument') rmSync(join(PKG_ROOT, triple), { recursive: true, force: true });
	console.log(`stage: ${phase} pair laid out under ${node_modules}`);
};

// --- 5. training ----------------------------------------------------------------------

/** A fresh copy of `collections` of the snapshot, rooted like it so discovery reads the
 * same scope: the pinned `.gitignore` written, and a `.git` marker making the copy its own
 * format root (tsv keys the git regime on `.git` existing; no git runs). */
const corpus_copy = (dest: string, collections: string[], corpus: Corpus): string => {
	rmSync(dest, { recursive: true, force: true });
	mkdirSync(join(dest, '.git'), { recursive: true });
	if (corpus.gitignore !== null) writeFileSync(join(dest, '.gitignore'), corpus.gitignore);
	for (const name of collections) {
		cpSync(join(corpora_root, CORPORA_TREE, name), join(dest, CORPORA_TREE, name), {
			recursive: true
		});
	}
	return join(dest, CORPORA_TREE);
};

/** The files `tsv format --list` finds under `root`, one absolute path each — each
 * required to exist, so a quoted or mangled spelling fails rather than drops a file. */
const list_files = (cmd: string[], root: string, env?: Record<string, string>): string[] => {
	const r = capture(cmd[0]!, [...cmd.slice(1), 'format', '--list', root], env);
	if (r.status !== 0)
		fail(`${cmd.join(' ')} format --list ${root} exited ${r.status}: ${r.stderr}`);
	const files = r.stdout
		.split(/\r?\n/)
		.filter(Boolean)
		.map((p) => resolve(p));
	const missing = files.filter((f) => !existsSync(f));
	if (missing.length > 0)
		fail(`format --list printed paths that do not exist: ${missing.slice(0, 5).join(', ')}`);
	if (files.length === 0) fail(`format --list found no files under ${root}`);
	return files;
};

/** Run `cmd` once per file (`args` before the path), collecting the files whose exit code
 * is not in `accept` — a training run must not quietly skip what it claims to train. */
const per_file = (
	cmd: string[],
	args: string[],
	files: string[],
	env: Record<string, string>,
	accept: readonly number[]
): string[] => {
	const failures: string[] = [];
	for (const file of files) {
		const r = spawnSync(cmd[0]!, [...cmd.slice(1), ...args, file], {
			stdio: ['ignore', 'ignore', 'pipe'],
			encoding: 'utf8',
			env: { ...process.env, ...env }
		});
		if (r.error) fail(`could not run ${cmd.join(' ')}: ${r.error.message}`);
		if (!accept.includes(r.status ?? -1)) {
			failures.push(`${file} (exit ${r.status ?? r.signal}): ${r.stderr.trim().split('\n')[0]}`);
		}
	}
	return failures;
};

const cmd_train = (): void => {
	const corpus = read_corpus();
	const instr = artifacts('instrument');
	require_file(instr.cli, 'run build.sh instrument first');
	const loader = loader_entries(paths.loader('instrument'));
	require_file(loader.cli, 'run stage --phase instrument first');
	const discard = { LLVM_PROFILE_FILE: join(paths.profraw_discard, '%m.profraw') };
	for (const world of WORLDS) {
		const root = corpus_copy(paths.train(world), corpus.own, corpus);
		const cmd = world === 'cli' ? [instr.cli] : [NODE, loader.cli];
		const files = list_files([instr.cli], root, discard);
		const raw = paths.profraw(world);
		rmSync(raw, { recursive: true, force: true });
		mkdirSync(raw, { recursive: true });
		const env = { LLVM_PROFILE_FILE: join(raw, `${world}-%4m.profraw`) };
		const t0 = Date.now();
		const by_language = (lang: string): string[] => files.filter((f) => parse_language(f) === lang);
		const strip_files = files.filter(is_strip_path);
		const trained: TrainedCounts = {
			files: files.length,
			typescript: by_language('typescript').length,
			svelte: by_language('svelte').length,
			css: by_language('css').length,
			strip: strip_files.length,
			imports_typescript: 0,
			imports_svelte: 0
		};
		// the parse (and the CLI's `loc` writer), on every file: each must parse
		const failures = per_file(cmd, ['parse'], files, env, [0]);
		if (world === 'cli') {
			failures.push(...per_file(cmd, ['parse', '--locations'], files, env, [0]));
			// the strip: 0 stripped, 1 refused — both verdicts; 2 is a parse error
			failures.push(...per_file(cmd, ['strip'], strip_files, env, [0, 1]));
		} else {
			// the strip and the import records, in process through the loader
			const list = join(paths.train(world), 'addon_ops.txt');
			writeFileSync(list, files.join('\n') + '\n');
			const ops = capture(
				NODE,
				[
					'scripts/pgo/addon_ops.ts',
					...['--loader', loader.index, '--list', list],
					...['--out', join(paths.train(world), 'addon_ops.ndjson')],
					...['--ops', 'strip,imports', '--require-ok']
				],
				env
			);
			process.stdout.write(ops.stdout);
			if (ops.status !== 0)
				fail(`train ${world}: the addon's strip or import records failed:\n${ops.stderr}`);
			trained.imports_typescript = trained.typescript;
			trained.imports_svelte = trained.svelte;
		}
		const format = capture(cmd[0]!, [...cmd.slice(1), 'format', '--jobs', '1', root], env);
		const raws = readdirSync(raw).filter((f) => f.endsWith('.profraw'));
		console.log(
			`train ${world}: ${files.length} files (${trained.strip} stripped), ${failures.length} failed, ` +
				`format exit ${format.status}, ${((Date.now() - t0) / 1000).toFixed(1)} s, ${raws.length} profraw files`
		);
		if (failures.length > 0) {
			fail(
				`train ${world}: ${failures.length} training runs failed:\n${failures.slice(0, 10).join('\n')}`
			);
		}
		if (format.status !== 0)
			fail(`train ${world}: format exited ${format.status}:\n${format.stderr.slice(0, 4000)}`);
		if (raws.length === 0 || raws.every((f) => statSync(join(raw, f)).size === 0)) {
			fail(
				`train ${world}: no profile was written to ${raw} — is ${world === 'cli' ? instr.cli : 'the staged addon'} instrumented?`
			);
		}
		writeFileSync(paths.trained(world), JSON.stringify(trained) + '\n');
	}
};

// --- 6. merge + the non-empty gate -----------------------------------------------------

/** The pinned toolchain's `llvm-tools-preview` binaries, from the host rustc's sysroot. */
const llvm_tools = (): { profdata: string; readobj: string } => {
	const vv = capture('rustc', ['-vV']);
	if (vv.status !== 0) fail(`rustc -vV failed: ${vv.stderr}`);
	const host = /^host: (\S+)$/m.exec(vv.stdout)?.[1] ?? fail('rustc -vV names no host');
	const sysroot = capture('rustc', ['--print', 'sysroot']).stdout.trim();
	const exe = process.platform === 'win32' ? '.exe' : '';
	const bin = join(sysroot, 'lib', 'rustlib', host, 'bin');
	const profdata = join(bin, `llvm-profdata${exe}`);
	const readobj = join(bin, `llvm-readobj${exe}`);
	for (const tool of [profdata, readobj]) {
		if (!existsSync(tool)) fail(`${tool} not found — rustup component add llvm-tools-preview`);
	}
	return { profdata, readobj };
};

const cmd_merge = (): void => {
	const { profdata } = llvm_tools();
	// the profile's format is the instrumenting compiler's LLVM — on a container row the
	// container's rustc, which `build.sh instrument` recorded — and the merge must read it
	const instrumented_llvm = llvm_version(
		readFileSync(require_file(paths.rustc_instrument, 'run build.sh instrument first'), 'utf8')
	);
	const tool_llvm = llvm_version(capture(profdata, ['--version']).stdout);
	if (!instrumented_llvm || instrumented_llvm !== tool_llvm) {
		fail(
			`llvm-profdata is LLVM ${tool_llvm}, the instrumenting rustc was LLVM ${instrumented_llvm} ` +
				`(${paths.rustc_instrument}) — a profile must be merged by its compiler's LLVM`
		);
	}
	for (const world of WORLDS) {
		const raw = paths.profraw(world);
		const raws = existsSync(raw) ? readdirSync(raw).filter((f) => f.endsWith('.profraw')) : [];
		if (raws.length === 0) fail(`no profraw files in ${raw} — run train first`);
		const trained = JSON.parse(
			readFileSync(require_file(paths.trained(world), 'run train first'), 'utf8')
		) as TrainedCounts;
		const out = paths.profdata(world);
		rmSync(out, { force: true });
		run(profdata, ['merge', '-o', out, ...raws.sort().map((f) => join(raw, f))]);
		const show = capture(profdata, ['show', '--all-functions', '--counts', out]);
		if (show.status !== 0) fail(`llvm-profdata show ${out} failed: ${show.stderr}`);
		const verdict = grade_profile(show.stdout, world, trained);
		console.log(
			`merge ${world}: ${verdict.records} function records, ${verdict.executed} executed; sha256 ${sha256_file(out)}`
		);
		for (const h of verdict.hot) {
			console.log(
				`  ${String(h.entry_count).padStart(10)} calls (floor ${h.floor}), max counter ${h.max_count}  ${h.path}`
			);
		}
		if (verdict.failures.length > 0)
			fail(`merge ${world}: the profile is not a trained one:\n  ${verdict.failures.join('\n  ')}`);
	}
};

// --- 8. the use builds' profile diagnostics --------------------------------------------

const cmd_gate_builds = (): void => {
	for (const world of WORLDS) {
		const log_path = require_file(paths.build_log('use', world), 'run build.sh use first');
		const log = readFileSync(log_path, 'utf8');
		// the flag reached every workspace crate's compile — without this, zero diagnostics
		// below could mean a profile that never reached the build
		const crate = world === 'cli' ? 'tsv_cli' : 'tsv_napi';
		const used = profile_use_crates(log, `${world}.profdata`);
		const unprofiled = [...PROFILED_CRATES, crate].filter((c) => !used.has(c));
		if (unprofiled.length > 0) {
			fail(
				`gate-builds ${world}: no -Cprofile-use=…${world}.profdata on the rustc command line of ` +
					`${unprofiled.join(', ')} — ${log_path}`
			);
		}
		// Registry crates compile with `--cap-lints allow`, which silences LLVM's warnings too,
		// so the diagnostics graded here are the workspace crates' (the registry ones are
		// source-pinned by Cargo.lock, so their profile cannot go stale apart from it).
		const diagnostics = parse_pgo_diagnostics(log);
		const unexpected = diagnostics.filter((d) => !d.build_script);
		const build_scripts = diagnostics.length - unexpected.length;
		console.log(
			`gate-builds ${world}: -Cprofile-use on ${used.size} crates; ${unexpected.length} profile ` +
				`diagnostics, ${build_scripts} from build scripts`
		);
		if (unexpected.length > 0) {
			fail(
				`gate-builds ${world}: the profile does not cover the build it was applied to (stale, or trained ` +
					`on other source) — ${log_path}:\n${unexpected
						.slice(0, 20)
						.map((d) => `  ${d.module}: ${d.message}`)
						.join('\n')}`
			);
		}
		// The napi build's own build script never runs in training, so a host build (whose
		// build scripts take RUSTFLAGS) always reports it missing — the proof that
		// `-pgo-warn-missing-function` reached LLVM and its warnings reach this reader, without
		// which a zero above would be vacuous. A cross build compiles its build scripts without
		// RUSTFLAGS, so the cross row has the flag-spelling check above alone.
		if (world === 'napi' && !rust_target && build_scripts === 0) {
			fail(
				`gate-builds napi: no build-script diagnostic in ${log_path} — did -pgo-warn-missing-function reach the build?`
			);
		}
	}
};

// --- 9. instrumentation sections -------------------------------------------------------

const cmd_gate_sections = (): void => {
	const { readobj } = llvm_tools();
	const sections = (path: string): string => {
		const r = capture(readobj, ['--sections', require_file(path, 'build it first')]);
		if (r.status !== 0) fail(`llvm-readobj --sections ${path} failed: ${r.stderr}`);
		return r.stdout;
	};
	for (const [phase, want] of [
		['instrument', true],
		['use', false]
	] as const) {
		for (const path of Object.values(artifacts(phase))) {
			const has = has_instrumentation_sections(sections(path));
			console.log(
				`gate-sections: ${path}: ${has ? 'instrumented' : 'no instrumentation sections'}`
			);
			if (has !== want) {
				fail(
					want
						? `gate-sections: the instrumented ${path} shows no instrumentation section — the check cannot see this object format`
						: `gate-sections: ${path} carries instrumentation sections — an instrumented binary must never ship`
				);
			}
		}
	}
};

// --- 11. byte identity ------------------------------------------------------------------

/** Every file under `root`, relative and `/`-separated, sorted. */
const walk = (root: string): string[] => {
	const out: string[] = [];
	const visit = (d: string): void => {
		for (const e of readdirSync(d, { withFileTypes: true })) {
			const p = join(d, e.name);
			if (e.isDirectory()) visit(p);
			else out.push(relative(root, p).split('\\').join('/'));
		}
	};
	visit(root);
	return out.sort();
};

/** One run's exit code and output with the arm's own root spelled `<root>`, lines sorted —
 * the order files finish in is not the contract, the set is. */
const normalized = (r: RunResult, root: string): string => {
	const spellings = [root, root.split('\\').join('/')];
	const clean = (text: string): string =>
		text
			.split(/\r?\n/)
			.map((line) => spellings.reduce((l, s) => l.split(s).join('<root>'), line))
			.sort()
			.join('\n');
	return `exit ${r.status}\n--- stdout\n${clean(r.stdout)}\n--- stderr\n${clean(r.stderr)}`;
};

/** Run `addon_ops.ts` over `files` through the loader at `index`, returning its records. */
const addon_ops = (
	index: string,
	files: string[],
	ops: string,
	name: string,
	env?: Record<string, string>
): AddonRecord[] => {
	const list = join(paths.byteid, `${name}.txt`);
	const out = join(paths.byteid, `${name}.ndjson`);
	writeFileSync(list, files.join('\n') + '\n');
	run(
		NODE,
		['scripts/pgo/addon_ops.ts', '--loader', index, '--list', list, '--out', out, '--ops', ops],
		env
	);
	return readFileSync(out, 'utf8')
		.split('\n')
		.filter(Boolean)
		.map((line) => JSON.parse(line) as AddonRecord);
};

const cmd_byteid = (): void => {
	const corpus = read_corpus();
	const reference = resolve(args.reference ?? cargo_artifacts('target', triple, rust_target).cli);
	require_file(reference, 'build the plain CLI first (cargo build --locked -p tsv_cli --release)');
	const pgo = artifacts('use');
	require_file(pgo.cli, 'run build.sh use first');
	const loader = loader_entries(paths.loader('use'));
	require_file(loader.cli, 'run stage --phase use first');
	const instrumented = loader_entries(paths.loader('instrument'));
	require_file(instrumented.index, 'run stage --phase instrument first');
	const fixtures = resolve('tests/fixtures');
	const all_collections = readdirSync(join(corpora_root, CORPORA_TREE));
	const arms: { name: string; cmd: string[] }[] = [
		{ name: 'plain', cmd: [reference] },
		{ name: 'cli', cmd: [pgo.cli] },
		{ name: 'addon', cmd: [NODE, loader.cli] }
	];
	rmSync(paths.byteid, { recursive: true, force: true });
	const mismatches: string[] = [];

	// the format half: the whole snapshot and the fixture tree, formatted in place
	const outputs = new Map<string, string>();
	for (const arm of arms) {
		const arm_root = join(paths.byteid, arm.name);
		const tree = corpus_copy(join(arm_root, 'corpus'), all_collections, corpus);
		const fixture_copy = join(arm_root, 'fixtures');
		cpSync(fixtures, fixture_copy, { recursive: true });
		mkdirSync(join(fixture_copy, '.git'), { recursive: true });
		const report: string[] = [];
		for (const target of [tree, fixture_copy]) {
			const r = capture(arm.cmd[0]!, [...arm.cmd.slice(1), 'format', target]);
			report.push(normalized(r, arm_root));
		}
		outputs.set(arm.name, report.join('\n'));
	}
	const plain_root = join(paths.byteid, 'plain');
	const plain_files = walk(plain_root);
	for (const arm of arms.slice(1)) {
		if (outputs.get(arm.name) !== outputs.get('plain')) {
			mismatches.push(
				`format ${arm.name}: exit codes or reported paths differ from the plain CLI's`
			);
		}
		const arm_root = join(paths.byteid, arm.name);
		const files = walk(arm_root);
		if (files.join('\n') !== plain_files.join('\n')) {
			mismatches.push(`format ${arm.name}: the formatted tree holds a different file set`);
			continue;
		}
		for (const f of plain_files) {
			if (!readFileSync(join(arm_root, f)).equals(readFileSync(join(plain_root, f)))) {
				mismatches.push(`format ${arm.name}: ${f}`);
			}
		}
	}
	console.log(`byteid format: ${plain_files.length} files per arm compared`);

	// the per-file half, over the pristine sources: every fixture input of the parse, strip
	// and import-record trees, and a stride of the snapshot
	const inputs = (tree: string): string[] =>
		walk(resolve(tree))
			.filter((f) => /(^|\/)input\.(svelte|ts|mts|cts|svelte\.ts|css)$/.test(f))
			.map((f) => join(resolve(tree), f));
	const fixture_inputs = [
		...inputs('tests/fixtures'),
		...inputs('tests/fixtures_strip'),
		...inputs('tests/fixtures_imports')
	];
	const snapshot = list_files([reference], join(corpora_root, CORPORA_TREE)).filter(
		(_, i) => i % PARSE_SAMPLE_STRIDE === 0
	);
	const sample = [...fixture_inputs, ...snapshot];
	const strip_sample = sample.filter(is_strip_path);
	const sha256 = (text: string): string => createHash('sha256').update(text).digest('hex');

	// the CLI: plain vs PGO, the whole run (exit, stdout, stderr); the plain CLI's answer is
	// also what the addon is held to below
	const plain_answers = new Map<string, RunResult & { hash: string }>();
	const cli_runs: [string, string[], string[]][] = [
		['parse', ['parse'], sample],
		['parse --locations', ['parse', '--locations'], sample],
		['strip', ['strip'], strip_sample]
	];
	for (const [name, cmd_args, files] of cli_runs) {
		for (const file of files) {
			const r = capture(reference, [...cmd_args, file]);
			const p = capture(pgo.cli, [...cmd_args, file]);
			if (name !== 'parse --locations') {
				plain_answers.set(`${name} ${file}`, { ...r, hash: sha256(r.stdout), stdout: '' });
			}
			if (r.status !== p.status || r.stdout !== p.stdout || r.stderr !== p.stderr) {
				mismatches.push(`${name} cli: ${file}`);
			}
		}
	}

	// the addon's parse and strip, in process, held to the plain CLI's answer for the file
	const addon_records = addon_ops(loader.index, sample, 'parse,strip', 'addon_pgo');
	const expected = sample.length + strip_sample.length;
	if (addon_records.length !== expected) {
		fail(`byteid: the addon answered ${addon_records.length} of ${expected} parse and strip calls`);
	}
	for (const rec of addon_records) {
		const ref =
			plain_answers.get(`${rec.op} ${rec.path}`) ??
			fail(`byteid: the addon answered an unasked ${rec.op} of ${rec.path}`);
		// a returned failure: a strip refusal is the CLI's exit 1, a parse failure its exit 1
		// from `parse` and 2 from `strip`; its message is in the CLI's stderr either way
		const failure_status = rec.op === 'strip' && rec.kind !== 'unsupported' ? 2 : 1;
		const agrees = rec.unreadable
			? ref.status === 2
			: rec.ok
				? ref.status === 0 && rec.hash === ref.hash
				: rec.ok === false &&
					ref.status === failure_status &&
					rec.message !== undefined &&
					ref.stderr.includes(rec.message);
		if (!agrees) {
			mismatches.push(
				`${rec.op} addon: ${rec.path} (cli exit ${ref.status}; addon ${JSON.stringify({ ...rec, path: undefined })})`
			);
		}
	}

	// the import records: no CLI has them, so the PGO addon is held to the instrumented one
	// — the same source and optimization without the profile (its counters written aside)
	const imports_sample = sample.filter((f) => parse_language(f) !== 'css');
	const pgo_imports = addon_ops(loader.index, imports_sample, 'imports', 'imports_pgo');
	const ref_imports = addon_ops(
		instrumented.index,
		imports_sample,
		'imports',
		'imports_reference',
		{
			LLVM_PROFILE_FILE: join(paths.profraw_discard, '%m.profraw')
		}
	);
	if (
		pgo_imports.length !== imports_sample.length ||
		ref_imports.length !== imports_sample.length
	) {
		fail(
			`byteid: the import records answered ${pgo_imports.length} / ${ref_imports.length} of ${imports_sample.length} files`
		);
	}
	for (const [i, rec] of pgo_imports.entries()) {
		if (JSON.stringify(rec) !== JSON.stringify(ref_imports[i])) {
			mismatches.push(`imports addon: ${rec.path}`);
		}
	}
	console.log(
		`byteid per file: ${sample.length} parsed, ${strip_sample.length} stripped, ` +
			`${imports_sample.length} import-recorded (${fixture_inputs.length} fixture inputs, every ` +
			`${PARSE_SAMPLE_STRIDE}th of ${CORPORA_TREE})`
	);
	if (mismatches.length > 0) {
		fail(
			`byteid: the PGO binaries' output differs from the reference's:\n  ${mismatches.slice(0, 20).join('\n  ')}`
		);
	}
	console.log("byteid: the PGO CLI's and addon's output is byte-identical to the plain builds'");
};

// --- 13. the depth limits in the staged addon ---------------------------------------------

/** The shapes the depth gate runs, from the native test (`write_shape_limits`). The limits
 * are counts, so any build writes the same lines: the `corpus` profile, the cheapest
 * optimized one, with the audit seams `deno task test:depth` builds it with, so a local run
 * after `deno task check` reuses that build; into `CARGO_TARGET_DIR` as the caller set it.
 * Always a HOST build — on a cross row too, since the shapes do not depend on the target. */
const cmd_depth_shapes = (): void => {
	mkdirSync(paths.depth, { recursive: true });
	if (
		!write_shape_limits(paths.depth_shapes, {
			cwd: process.cwd(),
			profile: 'corpus',
			features: ['tsv_debug/_internal_audits']
		})
	) {
		fail('depth-shapes: write_shape_limits failed');
	}
	require_file(paths.depth_shapes, 'write_shape_limits wrote nothing');
};

/**
 * On Windows, two scratch copies of node.exe under `<dir>/depth/`, each with its PE stack
 * reserve set just above one pass's request, so a worker's `stackSizeMb` sizes its thread
 * there as it does elsewhere — node.exe's own reserve would otherwise be every worker's
 * thread, or a request rounded up to a MiB: `node-control.exe` (`CONTROL_STACK_RESERVE`)
 * for the control pass and `node-gate.exe` (`GATE_STACK_RESERVE`) for the gate, whose main
 * thread runs `addon_depth.ts` itself. The real node.exe is only read. Each copy is
 * start-checked on `addon_depth.ts` with no arguments, which loads and type-strips its
 * module graph on that copy's main thread and must exit 2 with its usage line. Returns
 * both paths, or the reason the copies cannot be used (unreadable or unpatchable header, a
 * failed write, a copy that does not start), which leaves the row to the fallback.
 */
const node_copies = (): { control: string; gate: string } | { error: string } => {
	const probe = spawnSync(NODE, ['-p', 'process.execPath'], { encoding: 'utf8' });
	const node_exe = probe.status === 0 ? probe.stdout.trim() : '';
	if (!node_exe) return { error: `could not locate node.exe (${NODE} -p process.execPath)` };
	let image: Uint8Array;
	try {
		image = readFileSync(node_exe);
	} catch (e) {
		return { error: `could not read ${node_exe} (${(e as Error).message})` };
	}
	const sizes = pe_stack_sizes(image);
	if (!sizes) return { error: `could not read the PE stack fields of ${node_exe}` };
	console.log(
		`depth: ${node_exe} reserves ${sizes.reserve} B of stack (commit ${sizes.commit} B); ` +
			'each pass runs under a copy reserving just above its request'
	);
	const copies = { control: CONTROL_STACK_RESERVE, gate: GATE_STACK_RESERVE };
	const paths_out = {} as { control: string; gate: string };
	for (const [name, reserve] of Object.entries(copies) as Array<[keyof typeof copies, number]>) {
		const copy_image = image.slice();
		const set = pe_set_stack_reserve(copy_image, reserve);
		if (!set.ok) return { error: `could not set ${node_exe}'s stack reserve: ${set.reason}` };
		const copy = join(paths.depth, `node-${name}.exe`);
		try {
			mkdirSync(paths.depth, { recursive: true });
			writeFileSync(copy, copy_image);
		} catch (e) {
			return { error: `could not write the ${name} node.exe copy (${(e as Error).message})` };
		}
		const started = spawnSync(copy, ['scripts/pgo/addon_depth.ts'], { encoding: 'utf8' });
		if (started.error || started.status !== 2 || !started.stderr.includes('usage:')) {
			return {
				error:
					`the ${name} node.exe copy did not start addon_depth.ts ` +
					`(${started.error?.message ?? `exit ${started.status ?? started.signal}, expected its usage exit 2`}` +
					`${started.stderr?.trim() ? `: ${started.stderr.trim().slice(-500)}` : ''})`
			};
		}
		console.log(`depth: ${copy} reserves ${reserve} B and starts`);
		paths_out[name] = copy;
	}
	return paths_out;
};

/**
 * The depth gate over the staged PGO addon (`addon_depth.ts`). On Windows the gate runs
 * under `node_copies`' gate copy and the control pass under its control copy, whose stack
 * reserves let a worker's `stackSizeMb` size its thread. The row falls back to the
 * plain-profile native gate (`deno task test:depth:napi`'s run: the shipped optimization
 * without the profile, over the full sweep) on Windows alone, when a copy cannot be made or
 * does not start, or when `addon_depth.ts` exits `CANNOT_GRADE_EXIT` with
 * `CANNOT_GRADE_LINE` on stderr (the gate's thread would land above the contract, or the
 * control survived on a thread the reserve sized) — and says so, as a GitHub warning on a
 * workflow run: the PGO addon's own frames go ungraded there. Any other failure, and any
 * non-zero exit off Windows, fails.
 */
const cmd_depth = (): void => {
	const shapes = require_file(resolve(args.shapes ?? paths.depth_shapes), 'run depth-shapes first');
	const loader = loader_entries(paths.loader('use'));
	require_file(loader.index, 'run stage --phase use first');
	const win32 = process.platform === 'win32';
	const nodes = win32 ? node_copies() : { control: NODE, gate: NODE };
	const fallback_reason =
		'error' in nodes ? nodes.error : run_addon_depth(nodes, shapes, loader.index);
	if (fallback_reason === null) return;
	const message =
		`depth: FALLBACK — ${fallback_reason}; the staged PGO addon's depth is graded by the ` +
		'plain-profile native gate instead (test:depth:napi: cargo test --profile napi, TSV_DEPTH_SWEEP=full)';
	console.log(message);
	if (process.env.GITHUB_ACTIONS) console.log(`::warning title=depth fallback::${message}`);
	run(
		'cargo',
		[
			'test',
			'--locked',
			'--profile',
			'napi',
			'-p',
			'tsv_tests',
			'--test',
			'depth_limit',
			'--quiet'
		],
		{ TSV_DEPTH_SWEEP: 'full' }
	);
};

/** Run `addon_depth.ts` under `nodes.gate`, its control pass under `nodes.control`: `null`
 * when it graded and held, the fallback's reason when it cannot grade here (Windows only),
 * and a pipeline failure otherwise. */
const run_addon_depth = (
	nodes: { control: string; gate: string },
	shapes: string,
	loader_index: string
): string | null => {
	const depth_args = [
		'scripts/pgo/addon_depth.ts',
		...['--loader', loader_index, '--shapes', shapes, '--dir', join(paths.depth, 'run')],
		...(nodes.control === nodes.gate ? [] : ['--control-node', nodes.control])
	];
	console.log(`$ ${nodes.gate} ${depth_args.join(' ')}`);
	const r = spawnSync(nodes.gate, depth_args, {
		stdio: ['inherit', 'inherit', 'pipe'],
		encoding: 'utf8',
		maxBuffer: 1 << 28
	});
	if (r.stderr) process.stderr.write(r.stderr);
	const win32 = process.platform === 'win32';
	if (r.error) {
		// on Windows `nodes.gate` is the gate copy, which the start check ran once already;
		// any other spawn error (ENOBUFS from a flood of failure output) is the gate's own
		if (win32 && spawn_did_not_start(r.error)) {
			return `the gate node.exe copy did not start (${r.error.message})`;
		}
		fail(`could not run ${nodes.gate}: ${r.error.message}`);
	}
	if (r.status === 0) return null;
	if (win32 && r.status === CANNOT_GRADE_EXIT && r.stderr.includes(CANNOT_GRADE_LINE)) {
		return 'addon_depth.ts cannot grade the stack contract on this row (its reason above)';
	}
	return fail(`depth: the addon depth gate failed (exit ${r.status ?? r.signal})`);
};

// --- all ---------------------------------------------------------------------------------

const build_sh = (phase: string): void =>
	run('sh', ['scripts/pgo/build.sh', phase, dir, ...(rust_target ? [rust_target] : [])]);

/** Run one step, then print its wall time — the release rows' per-step durations, read off
 * a dispatch dry run to size the job's timeout. */
const timed = (name: string, step: () => void): void => {
	const started = performance.now();
	step();
	console.log(`pgo.ts ${name}: ${((performance.now() - started) / 1000).toFixed(1)} s`);
};

const cmd_all = (): void => {
	timed('corpora', () => cmd_corpora(false));
	timed('plain cli', () =>
		run('cargo', [
			'build',
			'--locked',
			'-p',
			'tsv_cli',
			'--release',
			...(rust_target ? ['--target', rust_target] : [])
		])
	);
	timed('build.sh instrument', () => build_sh('instrument'));
	timed('stage instrument', () => cmd_stage('instrument'));
	timed('train', cmd_train);
	timed('merge', cmd_merge);
	timed('build.sh use', () => build_sh('use'));
	timed('gate-builds', cmd_gate_builds);
	timed('gate-sections', cmd_gate_sections);
	timed('stage use', () => cmd_stage('use'));
	timed('byteid', cmd_byteid);
	timed('size bands', () =>
		run(DENO, [
			'run',
			'--allow-read',
			'scripts/validate_napi_artifact.ts',
			'--triple',
			triple,
			'--pgo'
		])
	);
	if (!args['no-depth']) {
		timed('depth-shapes', cmd_depth_shapes);
		timed('depth', cmd_depth);
	}
	console.log(`build:pgo: done — the PGO pair is staged in ${PKG_ROOT}/${triple}/`);
};

const commands: Record<string, () => void> = {
	corpora: () => cmd_corpora(args.fetch!),
	stage: () => {
		const phase = args.phase;
		if (phase !== 'instrument' && phase !== 'use') fail('stage needs --phase instrument|use');
		cmd_stage(phase as Phase);
	},
	train: cmd_train,
	merge: cmd_merge,
	'gate-builds': cmd_gate_builds,
	'gate-sections': cmd_gate_sections,
	byteid: cmd_byteid,
	'depth-shapes': cmd_depth_shapes,
	depth: cmd_depth,
	all: cmd_all
};

const [command, ...extra] = positionals;
const action = command ? commands[command] : undefined;
if (!action || extra.length > 0) {
	console.error(
		`usage: pgo.ts <${Object.keys(commands).join('|')}> [options] (see the module doc)`
	);
	process.exit(2);
}
try {
	// `all` times each of its steps; a single step (a release row's) times itself
	if (command === 'all') action();
	else timed(command!, action);
} catch (e) {
	if (!(e instanceof PipelineError)) throw e;
	console.error(`\nPGO FAILED: ${e.message}`);
	process.exit(1);
}

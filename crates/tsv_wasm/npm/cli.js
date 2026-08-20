#!/usr/bin/env node
/**
 * The `tsv` bin — mirrors the native `tsv_cli` contract (subcommands, flags,
 * exit codes, output streams, traversal rules) over whichever engine
 * `./index.js` resolves to: one source shipped verbatim in both
 * `@fuzdev/tsv_wasm` (WASM) and the native `@fuzdev/tsv` (N-API). The Rust
 * `tsv_cli` binary is still the fast path for large trees — the per-file
 * engine tax is the gap, not the driver.
 *
 * Path mode formats in parallel on `node:worker_threads` behind `--jobs`,
 * spawning this same file as its own worker (`isMainThread` splits the two
 * roles). A pool only pays for itself once the job is big enough to amortize
 * its startup, so the *default* waits for `WORKER_FILE_THRESHOLD` files; an
 * explicit `--jobs N` is obeyed at any size, as it is natively. `--jobs 1`,
 * `--content`/`--stdin`, and `--list` stay on one thread either way.
 * Which engine a worker binds is decided by whether the main thread's
 * `./index.js` exposes a `wasm_module`: the WASM package hands the compiled
 * module across and the worker initializes from it (compiled code is shared
 * process-wide, so no worker recompiles), while the N-API package has no such
 * export and its workers just load the addon. That same discriminant sizes the
 * pool — both the threshold and the worker count are per engine, because a WASM
 * run shares its cores with V8's wasm tier-up and a native one doesn't.
 *
 * Exit codes: `format` — 0 clean, 1 would-change (`--check`), 2 errors;
 * `parse` — 0 ok, 1 error. Flag-parsing errors exit 1 (both commands); `format`'s
 * post-parse usage/validation errors (an invalid `--goal`, conflicting inputs) exit 2.
 */

import {
	existsSync,
	readdirSync,
	readFileSync,
	realpathSync,
	statSync,
	writeFileSync
} from 'node:fs';
import { availableParallelism } from 'node:os';
import { dirname, isAbsolute, join, relative as path_relative, sep } from 'node:path';
import { parseArgs } from 'node:util';
import { isMainThread, parentPort, Worker, workerData } from 'node:worker_threads';

/** The compiled module a worker inherited from the main thread, or `undefined`
 * on the main thread and in an N-API worker (whose package exports none). */
const inherited_wasm_module = isMainThread ? undefined : workerData?.wasm_module;

/**
 * The engine, resolved before anything else runs.
 *
 * A worker handed a compiled `wasm_module` binds through `./worker.js`, the
 * package's no-auto-init entry, so it initializes from the module the main
 * thread already compiled instead of reading and compiling the `.wasm` again.
 * Every other role — the main thread, and an N-API worker, whose package
 * exports no `wasm_module` — loads `./index.js`, which auto-initializes. The
 * dynamic import is what makes that choice possible at all: a static one is
 * hoisted above the branch, so the worker would have paid for `./index.js`
 * before it could ask.
 */
const engine = await (async () => {
	if (inherited_wasm_module === undefined) return import('./index.js');
	const lazy = await import('./worker.js');
	lazy.init_sync({ module: inherited_wasm_module });
	return lazy;
})();

/**
 * Which engine this copy is bound to. The same `wasm_module` export that
 * decides how a worker binds also decides how the pool is *sized*, because the
 * two engines have different amounts of machine left to give a worker: see
 * `WORKER_FILE_THRESHOLD` and `default_jobs`.
 *
 * Answered per role rather than from `engine` alone — a WASM worker binds
 * `./worker.js`, which exports no `wasm_module` of its own, so asking the
 * engine there would say "native".
 */
const IS_WASM_ENGINE = isMainThread
	? engine.wasm_module !== undefined
	: inherited_wasm_module !== undefined;

const { IgnoreStack } = engine;

/** tsv's native ignore file, discovered hierarchically (one per directory).
 * Mirrors `FORMATIGNORE_FILE`. */
const FORMATIGNORE_FILE = '.formatignore';

/** Prettier's ignore file — read hierarchically inside a git repo (one per
 * directory, like `.formatignore`) for drop-in compat, and shadowed by *presence*
 * of a *sibling* `.formatignore` (used alone for that directory when present, even
 * if present-but-unreadable); the shadow is flagged by a heads-up. Never read
 * outside a repo — there a target-root `.prettierignore` triggers a heads-up
 * warning instead. Mirrors `PRETTIERIGNORE_FILE`. */
const PRETTIERIGNORE_FILE = '.prettierignore';

/** The git ignore file, discovered hierarchically (one per directory) and only
 * inside a git repo, matching git. */
const GITIGNORE_FILE = '.gitignore';

const FORMATTERS = {
	svelte: engine.format_svelte,
	typescript: engine.format_typescript,
	css: engine.format_css
};

const PARSERS = {
	svelte: engine.parse_svelte_json,
	typescript: engine.parse_typescript_json,
	css: engine.parse_css_json
};

/**
 * How many in-scope files it takes before worker threads are worth their
 * startup. Below this the run stays on the main thread: bringing a pool up
 * costs tens of milliseconds (module resolution and per-isolate setup, paid
 * once per worker), which a small job never earns back — and unlike the native
 * CLI's ~50µs thread spawn, that cost is large enough to have to gate on.
 * Clamping the worker count to the file count (as the native CLI does) is not
 * enough on its own: two workers over four files is still a losing trade.
 *
 * One value per engine, because the crossover is not a property of the driver.
 * Measured by sweeping file count against `--jobs 1` on a real tree (outline's
 * 1648-file TypeScript subset, size-stratified subsets, quiet machine, 6
 * physical cores): the pool breaks even at ~565 files on WASM and ~394 on
 * N-API. Both constants sit above their crossover on purpose — below it a pool
 * costs wall time *and* ~1.5× the CPU and RSS, while above it staying serial
 * costs only wall time, so being late is the cheaper error.
 *
 * File count is only a proxy for work: one 500 KB file outweighs a hundred
 * small ones and still stays on this thread, which is correct anyway (a single
 * file can't be split), but it means these can only ever be approximately
 * right. A byte-count gate would track the real quantity, and was declined: it
 * needs a `stat` per discovered file, which `readdirSync(withFileTypes)` does
 * not give for free.
 */
const WASM_WORKER_FILE_THRESHOLD = 768;
const NATIVE_WORKER_FILE_THRESHOLD = 512;
const WORKER_FILE_THRESHOLD = IS_WASM_ENGINE
	? WASM_WORKER_FILE_THRESHOLD
	: NATIVE_WORKER_FILE_THRESHOLD;

/** Valid `--parser` values (shared by `format` and `parse` — `FORMATTERS` and
 * `PARSERS` are keyed by the same names). */
const PARSER_NAMES = new Set(['svelte', 'typescript', 'css']);

const HELP = `Usage: tsv <command> [<args>]

formatter and parser for Svelte, TypeScript, and CSS

Options:
  --version         print the tsv version

Commands:
  format            Format source code in place (near-Prettier output)
  parse             Parse source code into AST JSON
  help              Print help for a command

Run \`tsv <command> --help\` for command flags.
`;

const FORMAT_HELP = `Usage: tsv format [<paths...>] [--check] [--list] [--content <s> | --stdin] [--parser <p>] [--goal <g>]

Format source code in place (near-Prettier output).

Paths are formatted in place (written only when the output differs) and
changed paths print to stdout; directories recurse over
.ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css, honoring .gitignore
(hierarchically, in a git tree) plus hierarchical .formatignore /
.prettierignore. An explicitly named file skips the ignore
files, but its extension must still be one tsv formats.
--content/--stdin print formatted source to stdout.

Options:
  --content <s>     content to format, printed to stdout (requires --parser)
  --stdin           read from stdin, print to stdout (requires --parser)
  --parser <p>      parser type: svelte | typescript | css (--content/--stdin only)
  --goal <g>        TypeScript parse goal: script | module (default: module; --content/--stdin only)
  --check           check instead of writing/printing: exit 1 if any input would change
  --list            list the discovered in-scope files (one per line) without formatting; path mode only
  --jobs <n>        worker thread count (default: scaled to this machine and engine)

Exit codes: 0 clean, 1 would change (--check), 2 errors.
`;

const PARSE_HELP = `Usage: tsv parse [<file>] [--pretty] [--content <s> | --stdin] [--parser <p>] [--goal <g>] [--no-locations]

Parse source code into AST JSON.

Options:
  --pretty          pretty-print JSON output
  --content <s>     content to parse (requires --parser)
  --stdin           read from stdin (requires --parser)
  --parser <p>      parser type: svelte | typescript | css
  --goal <g>        TypeScript parse goal: script | module (default: module)
  --no-locations    omit per-node loc (span-only wire; svelte also omits name_loc; no-op for css)
`;

// the two roles this file plays: the CLI itself, and the worker it spawns for
// path-mode formatting (see `format_files_parallel`)
if (isMainThread) {
	await main();
} else {
	run_format_worker();
}

async function main() {
	const [command, ...rest] = process.argv.slice(2);
	switch (command) {
		case 'format':
			await run_format(rest);
			break;
		case 'parse':
			run_parse(rest);
			break;
		case 'help':
			run_help(rest);
			break;
		case '--help':
			print(HELP);
			break;
		case '--version':
			print_version();
			break;
		case undefined:
			eprint(HELP);
			process.exit(1);
			break;
		default:
			eprint(`Error: unknown command '${command}'\n\n${HELP}`);
			process.exit(1);
	}
}

/** `tsv --version` — mirrors the native CLI's top-level version switch, exact
 * output shape (`tsv <version>`). The version is this package's own — read
 * lazily from the sibling package.json (cli.js ships at the package root of
 * both `@fuzdev/tsv_wasm` and `@fuzdev/tsv`, and the published sets move in
 * version lockstep with the native binary). */
function print_version() {
	const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf-8'));
	print(`tsv ${pkg.version}\n`);
}

/** `tsv help [command]` — mirrors the native CLI's argh-generated help subcommand. */
function run_help(args) {
	switch (args[0]) {
		case undefined:
			print(HELP);
			break;
		case 'format':
			print(FORMAT_HELP);
			break;
		case 'parse':
			print(PARSE_HELP);
			break;
		default:
			eprint(`Unrecognized argument: ${args[0]}\n`);
			process.exit(1);
	}
}

/** Parse argv with `parseArgs`, exiting 1 on unknown/malformed flags. */
function parse_argv(args, options, help) {
	let parsed;
	try {
		parsed = parseArgs({ args, options, allowPositionals: true, strict: true });
	} catch (error) {
		eprint(`Error: ${error.message}\n`);
		process.exit(1);
	}
	if (parsed.values.help) {
		print(help);
		process.exit(0);
	}
	return parsed;
}

/** Resolve a `--parser` value (`ts` is an accepted alias), or exit 1 — the
 * native CLI validates the value at the argument-parsing layer (argh), so a
 * bad value is an argument-parsing error in every mode of both commands. */
function resolve_parser(name) {
	const resolved = name === 'ts' ? 'typescript' : name;
	if (!PARSER_NAMES.has(resolved)) {
		eprint(`Error: Unknown parser type: '${name}'. Valid types: svelte, typescript, css\n`);
		process.exit(1);
	}
	return resolved;
}

/** Validate a `--goal` value (the TypeScript goal axis), or exit `code` — the
 * native CLI validates it at the argument layer (`parse_goal_arg`), so a bad
 * value is an argument error (`parse` exits 1, `format` exits 2). Absent → `undefined`
 * (the `module` default); the goal only affects the TypeScript parser. */
function resolve_goal(goal, code) {
	if (goal === undefined || goal === 'module' || goal === 'script') return goal;
	eprint(`Error: invalid --goal '${goal}' (expected 'script' or 'module')\n`);
	process.exit(code);
}

/** The WASM `goal` option for `parser`. The option is TypeScript-only in the
 * WASM API (svelte's `<script>` is always a module, css has no goal), so a
 * validated-but-inert `--goal` on the other languages is spelled `undefined`
 * rather than passed — a supported key set to `undefined` reads as its default,
 * which is what lets one bag serve whichever parser or formatter (native-CLI
 * parity, since there too the goal only reaches TypeScript). */
function goal_option(parser, goal) {
	return parser === 'typescript' ? goal : undefined;
}

/** Extension-based parser detection, mirroring the native `ParserType::from_extension`. */
function parser_from_extension(path) {
	if (path.endsWith('.svelte')) return 'svelte';
	if (path.endsWith('.css')) return 'css';
	return 'typescript';
}

async function run_format(args) {
	const { values, positionals } = parse_argv(
		args,
		{
			content: { type: 'string' },
			stdin: { type: 'boolean' },
			parser: { type: 'string' },
			goal: { type: 'string' },
			check: { type: 'boolean' },
			list: { type: 'boolean' },
			jobs: { type: 'string' },
			help: { type: 'boolean' }
		},
		FORMAT_HELP
	);

	// validated before mode dispatch — a bad value exits 1 in every mode (argh parity)
	const parser = values.parser === undefined ? undefined : resolve_parser(values.parser);
	// --jobs sets the worker-thread count in path mode: a non-integer value is
	// an argument-parsing error (exit 1, argh parity), and combining it with
	// --content/--stdin is rejected in format_single like the native CLI (exit 2)
	if (values.jobs !== undefined && !/^\d+$/.test(values.jobs)) {
		eprint(`Error: --jobs expects an integer, got '${values.jobs}'\n`);
		process.exit(1);
	}
	if (values.content !== undefined || values.stdin) {
		// --goal is content/stdin-only and TypeScript-only; a bad value is a usage
		// error (exit 2, format parity). Path mode rejects --goal in format_paths.
		const goal = resolve_goal(values.goal, 2);
		format_single(values, positionals, parser, goal);
	} else {
		await format_paths(values, positionals);
	}
}

/** `--content`/`--stdin` mode — format one input to stdout (or `--check` it). */
function format_single(values, positionals, parser, goal) {
	if (positionals.length > 0) {
		eprint('Error: --content/--stdin cannot be combined with file paths\n');
		process.exit(2);
	}
	if (values.jobs !== undefined) {
		eprint('Error: --jobs applies to file paths; --content/--stdin format a single input\n');
		process.exit(2);
	}
	if (values.list) {
		eprint('Error: --list applies to file paths; --content/--stdin format a single input\n');
		process.exit(2);
	}
	const flag = values.content !== undefined ? '--content' : '--stdin';
	if (parser === undefined) {
		eprint(`Error: ${flag} requires --parser <svelte|typescript|css>\n`);
		process.exit(2);
	}
	const input = values.content !== undefined ? values.content : read_stdin(2);
	let formatted;
	try {
		// one bag, handed to whichever formatter
		formatted = FORMATTERS[parser](input, { goal: goal_option(parser, goal) });
	} catch (error) {
		eprint(`Parse error: ${error.message}\n`);
		process.exit(2);
	}
	if (values.check) {
		if (formatted !== input) {
			eprint('would change\n');
			process.exit(1);
		}
	} else {
		print(formatted);
	}
}

/** Path mode — discover files, format (in parallel above
 * `WORKER_FILE_THRESHOLD`), report in sorted order. */
async function format_paths(values, positionals) {
	if (positionals.length === 0) {
		eprint('Error: No input provided. Use a file path, --content, or --stdin\n');
		process.exit(2);
	}
	if (values.parser !== undefined) {
		eprint('Error: --parser applies to --content/--stdin; file paths use extension detection\n');
		process.exit(2);
	}
	if (values.goal !== undefined) {
		eprint('Error: --goal applies to --content/--stdin; file paths are formatted as modules\n');
		process.exit(2);
	}
	if (values.list && values.check) {
		eprint('Error: --list and --check cannot be combined\n');
		process.exit(2);
	}

	const { files, errors: traversal_errors, warnings } = discover_files(positionals);
	for (const msg of traversal_errors) {
		eprint(`error: ${msg}\n`);
	}
	// discovery warnings (e.g. the heuristic-shadow no-op) go to stderr but are
	// NOT errors — no effect on the exit code or stdout, so --list/--check output
	// stays clean. Fires in every path mode.
	for (const msg of warnings) {
		eprint(`warning: ${msg}\n`);
	}
	// --list reports the in-scope set and stops — no formatting, and an empty
	// result is a valid answer (exit 0), unlike the format action below which
	// treats "nothing found" as a usage error.
	if (values.list) {
		for (const path of files) {
			print(`${path}\n`);
		}
		if (traversal_errors.length > 0) process.exit(2);
		return;
	}
	if (files.length === 0 && traversal_errors.length === 0) {
		// neutral wording: an empty result can mean "no
		// .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css here" *or* "all of them are
		// ignored" (e.g. a target under a gitignored dir), so don't imply a
		// wrong-extension cause. `--list` reports the empty set and exits 0.
		eprint(
			'Error: No files to format — no unignored .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css files in scope\n'
		);
		process.exit(2);
	}

	const jobs = resolve_jobs(values.jobs, files.length);
	const outcomes =
		jobs > 1
			? await format_files_parallel(files, values.check, jobs)
			: format_files(files, values.check);

	// Buffer the changed-path lines and emit them in one write, matching the
	// native CLI: a per-path write re-locks stdout for each of (potentially
	// thousands of) changed files, which dominates --check on a large
	// unformatted tree. Errors stay per-line on stderr (rare, and stderr is for
	// immediate diagnostics). Both report in sorted-path order — `files` is
	// sorted and `outcomes` is indexed by it, so parallel execution reports
	// exactly what the sequential path does.
	let changed = 0;
	let unchanged = 0;
	let errors = traversal_errors.length;
	let changed_paths = '';
	for (let i = 0; i < files.length; i++) {
		const outcome = outcomes[i];
		if (outcome.kind === 'unchanged') {
			unchanged++;
		} else if (outcome.kind === 'changed') {
			changed++;
			changed_paths += `${files[i]}\n`;
		} else {
			errors++;
			eprint(`error: ${files[i]}: ${outcome.message}\n`);
		}
	}
	print(changed_paths);

	const action = values.check ? 'would change' : 'formatted';
	const error_note = errors > 0 ? `, ${errors} errors` : '';
	eprint(`${changed} ${action}, ${unchanged} unchanged${error_note}\n`);

	if (errors > 0) process.exit(2);
	if (values.check && changed > 0) process.exit(1);
}

/**
 * Format one discovered file, in place unless `check`. The single definition of
 * what a file's outcome is — the sequential path and every worker call it, so
 * the two can't drift on what counts as changed or how an error reads.
 */
function format_one(path, check) {
	let source;
	try {
		source = readFileSync(path, 'utf-8');
	} catch (error) {
		return { kind: 'error', message: `read failed: ${error.message}` };
	}
	let formatted;
	try {
		formatted = FORMATTERS[parser_from_extension(path)](source);
	} catch (error) {
		// TODO: a WASM stack overflow (input nested past ~300 levels — generated or
		// minified code) is not a per-file failure like a parse error is: the trap
		// leaves `__stack_pointer` where the deep call left it, so this instance is
		// poisoned and every later file throws `memory access out of bounds` too. On
		// this path that turns the rest of the run into spurious errors; the worker
		// path is contained only because the throw kills the worker and the shared
		// cursor lets the survivors drain the list. Recovering needs a fresh instance,
		// which `init_sync` does not give (wasm-bindgen's `initSync` short-circuits
		// once initialized) — so the fix is either a re-instantiation hook out of the
		// package patcher, or reporting the remainder as engine-reset rather than as
		// per-file errors.
		return { kind: 'error', message: error.message };
	}
	if (formatted === source) return { kind: 'unchanged' };
	if (!check) {
		try {
			writeFileSync(path, formatted);
		} catch (error) {
			return { kind: 'error', message: `write failed: ${error.message}` };
		}
	}
	return { kind: 'changed' };
}

/** Format every file on this thread — the path below `WORKER_FILE_THRESHOLD`,
 * and the fallback when a pool can't be brought up. */
function format_files(files, check) {
	return files.map((path) => format_one(path, check));
}

/** Logical and physical CPU counts. The SMT width is one read of
 * `thread_siblings_list`, not one per CPU; anywhere that read fails — every
 * non-Linux platform included — it reads 1 and physical degrades to logical,
 * so the topology can only ever lower a worker count, never raise it. */
function cpu_topology() {
	let logical;
	try {
		logical = availableParallelism();
	} catch {
		return { logical: 1, physical: 1 };
	}
	let siblings = 1;
	try {
		siblings = cpu_list_len(
			readFileSync('/sys/devices/system/cpu/cpu0/topology/thread_siblings_list', 'utf-8')
		);
	} catch {
		// unreadable (not Linux, or a kernel without the topology sysfs) — no SMT cap
	}
	return { logical, physical: Math.max(1, Math.floor(logical / Math.max(1, siblings))) };
}

/**
 * Worker count when `--jobs` is not given, clamped to the file count.
 *
 * Deliberately **not** the native CLI's `min(logical, ceil(1.5 × physical))`.
 * That rule assumes the pool arrives on an otherwise idle machine, and neither
 * engine here does — measured on a 6-physical-core machine over outline's
 * 1648-file tree, both peak well below it and *regress* past their peak:
 *
 * | engine | 1 | 2 | 3 | 4 | 6 | 9 | 12 |
 * | --- | --- | --- | --- | --- | --- | --- | --- |
 * | WASM  | 1.00× | 1.31× | **1.40×** | 1.37× | 1.32× | 1.21× | 1.14× |
 * | N-API | 1.00× | 1.34× | 1.64× | 1.82× | **1.90×** | 1.69× | 1.46× |
 *
 * The two knees have different causes, so they get different rules rather than
 * one fitted number:
 *
 * - **WASM — half the physical cores, because V8 already took the other half.**
 *   Wasm tier-up is itself multithreaded and starts before the first worker
 *   exists: at `--jobs 1` the process already runs at 2.39× CPU/wall, and
 *   tier-up accounts for 3.07G of the run's 8.34G instructions. Format workers
 *   are therefore competing with TurboFan, not filling idle cores. (With
 *   `node --liftoff-only` the same pool scales to 2.31× at 6 workers — which is
 *   the measurement, not an option: the flag can't be set from inside the
 *   process, and Bun has no equivalent.)
 * - **N-API — the physical cores, and no more.** No compiler thread to compete
 *   with (CPU/wall at `--jobs 1` is 1.02), so it scales cleanly to the core
 *   count. It still falls short of the native binary's 1.5× rule because this
 *   driver reads and decodes each file in JS, and that allocation pressure is
 *   per-worker GC the Rust pool never pays.
 */
function default_jobs() {
	const { logical, physical } = cpu_topology();
	return IS_WASM_ENGINE ? Math.max(1, Math.floor(physical / 2)) : Math.min(logical, physical);
}

/** One CPU index, or `undefined` if the field isn't a bare integer. Not
 * `Number()`, which reads the empty string as 0 — a trailing comma would then
 * count as a sibling. Mirrors Rust's `parse::<usize>()`, which rejects it. */
function cpu_index(field) {
	return /^\s*\d+\s*$/.test(field) ? Number.parseInt(field, 10) : undefined;
}

/** Length of a Linux CPU-list string (`"0-1"`, `"0,6"`, `"0-3,8-11"`, `"0"`).
 * Any malformed field contributes 0, so a surprise format degrades to "no SMT"
 * rather than to a bogus width. Mirrors the native `cpu_list_len`. */
function cpu_list_len(list) {
	let total = 0;
	for (const part of list.trim().split(',')) {
		const [lo, hi] = part.split('-');
		const low = cpu_index(lo);
		if (low === undefined) continue;
		if (hi === undefined) {
			total += 1;
			continue;
		}
		const high = cpu_index(hi);
		if (high !== undefined && high >= low) total += high - low + 1;
	}
	return total;
}

/**
 * How many workers to actually spawn: 1 means "stay on this thread".
 *
 * `WORKER_FILE_THRESHOLD` governs the **default** only. An explicit `--jobs N`
 * is obeyed at any file count — the user asked, the native CLI obeys it, and
 * second-guessing a flag is the kind of surprise a drop-in mirror can't afford.
 * It is also the only way to compare the two paths at a fixed size, which is
 * what calibrating the threshold and `default_jobs` took.
 *
 * Both forms clamp to the file count (a worker with no file to claim is pure
 * startup cost) and to a floor of 1, so `--jobs 0` means the same as `--jobs 1`
 * — matching the native clamp, which `--jobs 0` had escaped on its streaming
 * path (`format_streamed`).
 */
function resolve_jobs(requested, file_count) {
	if (requested === undefined) {
		return file_count < WORKER_FILE_THRESHOLD ? 1 : clamp_jobs(default_jobs(), file_count);
	}
	return clamp_jobs(Number.parseInt(requested, 10), file_count);
}

/** A worker count clamped to what there is work for, floored at 1. */
function clamp_jobs(jobs, file_count) {
	return Math.max(1, Math.min(jobs, file_count));
}

/**
 * Format `files` across `jobs` worker threads, returning outcomes indexed by
 * `files`.
 *
 * Work is claimed dynamically rather than partitioned up front: every worker
 * takes the next unclaimed index off one shared counter, so a slow file holds
 * up only itself. That mirrors the native pool's claim-by-index design, and for
 * the same reason — a static split strands whole workers behind one large file.
 *
 * A pool that cannot be brought up at all falls back to this thread. A worker
 * that dies mid-run does not: every index left unfilled surfaces as an error,
 * because turning one worker's death into a silent "unchanged" would report a
 * clean run over files nobody formatted. A dying worker still posts what it
 * finished (see `run_format_worker`), so "unfilled" means genuinely unformatted
 * rather than "claimed by the worker that died".
 */
async function format_files_parallel(files, check, jobs) {
	const outcomes = new Array(files.length);
	// one Int32 the workers bump with Atomics — the whole hand-off, since the
	// file list itself never changes once discovery is done
	const cursor = new Int32Array(new SharedArrayBuffer(4));
	// Every worker gets the identical payload — the cursor is what makes them
	// take different work. `wasm_module` is `undefined` on the N-API path, which
	// is exactly the signal a worker reads to load `./index.js` instead of the
	// wasm `./worker` entry.
	const worker_data = { wasm_module: engine.wasm_module, files, check, cursor };
	const workers = [];
	for (let i = 0; i < jobs; i++) {
		try {
			workers.push(new Worker(new URL(import.meta.url), { workerData: worker_data }));
		} catch (error) {
			// A worker starts claiming the moment it is constructed, so once ANY
			// came up, falling back to this thread would re-format files that are
			// already done — reporting them "unchanged" when they changed. A
			// narrower pool is not a problem: the cursor is shared, so however few
			// workers exist between them drain the whole list.
			if (workers.length > 0) {
				eprint(`warning: only ${workers.length} of ${jobs} format workers started\n`);
				break;
			}
			eprint(
				`warning: could not start format workers (${error.message}); formatting on one thread\n`
			);
			return format_files(files, check);
		}
	}

	await Promise.all(
		workers.map(
			(worker) =>
				new Promise((resolve) => {
					worker.on('message', (results) => {
						for (const { index, outcome } of results) outcomes[index] = outcome;
					});
					worker.on('error', (error) => {
						// not counted here — the unfilled-slot sweep below turns
						// every index nobody reported into its own error, which is
						// the accounting the summary line reports. A worker that
						// throws still posts what it finished on the way out, so
						// that sweep lands only on genuinely unformatted files.
						eprint(`error: format worker failed: ${error.message}\n`);
						resolve();
					});
					worker.on('exit', () => resolve());
				})
		)
	);

	for (let i = 0; i < outcomes.length; i++) {
		outcomes[i] ??= { kind: 'error', message: 'not formatted (worker failed)' };
	}
	return outcomes;
}

/**
 * The worker role: claim indices off the shared cursor until the list is
 * exhausted, then report every outcome in one message. Batching the report
 * keeps the message count at one per worker instead of one per file — the
 * results are tiny and the main thread has nothing to do with them until every
 * worker is done.
 *
 * The `finally` is what makes batching free of its usual cost. Reporting only
 * at the end would mean a worker that dies mid-run loses the files it *did*
 * finish along with the ones it didn't, and in place-formatting mode those were
 * already written — the run would claim a file wasn't formatted when it was on
 * disk. Posting on the way out of the throw keeps the accounting honest without
 * a per-file message: the send is enqueued before the exception propagates, so
 * the parent gets the partial batch and only then the `error` event (verified
 * on Node and Bun). A hard abort — OOM, a kill — still loses the batch, and
 * there the conservative over-report is the right answer anyway.
 */
function run_format_worker() {
	const { files, check, cursor } = workerData;
	const results = [];
	try {
		for (;;) {
			const index = Atomics.add(cursor, 0, 1);
			if (index >= files.length) break;
			results.push({ index, outcome: format_one(files[index], check) });
		}
	} finally {
		parentPort.postMessage(results);
	}
}

function run_parse(args) {
	const { values, positionals } = parse_argv(
		args,
		{
			pretty: { type: 'boolean' },
			content: { type: 'string' },
			stdin: { type: 'boolean' },
			parser: { type: 'string' },
			goal: { type: 'string' },
			'no-locations': { type: 'boolean' },
			help: { type: 'boolean' }
		},
		PARSE_HELP
	);

	// validated before mode dispatch — a bad value exits 1 in every mode (argh parity)
	const flag_parser = values.parser === undefined ? undefined : resolve_parser(values.parser);
	// --goal is validated upfront (exit 1) like the native CLI; it only affects the
	// TypeScript parser (svelte is always a module, css has no goal).
	const goal = resolve_goal(values.goal, 1);
	if (positionals.length > 1) {
		eprint(`Unrecognized argument: ${positionals[1]}\n`);
		process.exit(1);
	}

	// Input precedence mirrors the native `InputArgs::resolve`: --content > --stdin > file.
	let input;
	let parser;
	if (values.content !== undefined) {
		if (flag_parser === undefined) {
			eprint('Error: --content requires --parser <svelte|typescript|css>\n');
			process.exit(1);
		}
		parser = flag_parser;
		input = values.content;
	} else if (values.stdin) {
		if (flag_parser === undefined) {
			eprint('Error: --stdin requires --parser <svelte|typescript|css>\n');
			process.exit(1);
		}
		parser = flag_parser;
		input = read_stdin(1);
	} else if (positionals.length > 0) {
		const path = positionals[0];
		parser = flag_parser === undefined ? parser_from_extension(path) : flag_parser;
		try {
			input = readFileSync(path, 'utf-8');
		} catch (error) {
			eprint(`Error: Error reading file '${path}': ${error.message}\n`);
			process.exit(1);
		}
	} else {
		eprint('Error: No input provided. Use a file path, --content, or --stdin\n');
		process.exit(1);
	}

	// --no-locations drops per-node `loc` (span-only wire; svelte also `name_loc`,
	// a no-op for css); orthogonal to --goal (goal drives the TS parser,
	// no-locations the writer), so they compose. `locations` is a parse-only
	// option — format emits no wire and rejects the key.
	const no_locations = values['no-locations'] === true;
	let json;
	try {
		json = PARSERS[parser](input, {
			locations: !no_locations,
			goal: goal_option(parser, goal)
		});
	} catch (error) {
		eprint(`Parse error: ${error.message}\n`);
		process.exit(1);
	}
	if (values.pretty) {
		// re-serializing through the engine isn't guaranteed byte-identical to the
		// native CLI's to_json_with_tabs: number formatting diverges at extremes
		// (ryu emits `1e21`, ECMA-262 emits `1e+21`); compact output (the default)
		// is the verbatim wire string from Rust and exact
		json = JSON.stringify(JSON.parse(json), null, '\t');
	}
	print(`${json}\n`);
}

/** Synchronous stdout write — `process.stdout.write` is async on pipes, so a
 * `process.exit` right after it can truncate output. Safe from EAGAIN only
 * while nothing in the process initializes the `process.stdout`/`process.stderr`
 * streams (including via `console.*`) — stream init flips the fd to
 * non-blocking, after which a sync write to a full pipe can throw. */
function print(text) {
	writeFileSync(1, text);
}

/** Synchronous stderr write (same truncation hazard as `print`). */
function eprint(text) {
	writeFileSync(2, text);
}

/** Read all of stdin, exiting with the calling command's error code on
 * failure (`format` uses 2, `parse` uses 1 — mirroring the native CLI). */
function read_stdin(exit_code) {
	try {
		return readFileSync(0, 'utf-8');
	} catch (error) {
		eprint(`Error: Error reading from stdin: ${error.message}\n`);
		process.exit(exit_code);
	}
}

/** Read an ignore file, classifying the outcome so the walk can surface a
 * silently-dropped file and keep precedence by presence. Returns `{kind:
 * 'content', content}` on success, `{kind: 'absent'}` for ENOENT (missing, or
 * raced away after the listing — silent), or `{kind: 'unreadable'}` for any other
 * failure, pushing a non-fatal warning. **Strict UTF-8** (via `TextDecoder` with
 * `fatal`) to match Rust's `read_to_string` — Node's `readFileSync(path, 'utf-8')`
 * would lossily replace invalid bytes, silently applying a mangled ignore file the
 * native CLI drops. `ignoreBOM: true` *keeps* a leading BOM (the option name is
 * inverted) so a BOM-prefixed file decodes identically to Rust's `read_to_string`,
 * which doesn't strip it either. Mirrors the native `read_ignore_file`. */
function read_ignore_file(path, warnings) {
	let buf;
	try {
		buf = readFileSync(path);
	} catch (error) {
		if (error.code === 'ENOENT') return { kind: 'absent' };
		warnings.push(`could not read ${path} (${error.message}); its ignore rules are not applied`);
		return { kind: 'unreadable' };
	}
	try {
		const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
		return { kind: 'content', content: decoder.decode(buf) };
	} catch {
		warnings.push(`could not read ${path} (invalid UTF-8); its ignore rules are not applied`);
		return { kind: 'unreadable' };
	}
}

/** The nearest ancestor of `start` (inclusive) holding a `.git` entry (dir or
 * file) — the repo root — or null if there is no git tree above `start`.
 * Mirrors the native `find_repo_root`. */
function find_repo_root(start) {
	let dir = start;
	for (;;) {
		if (existsSync(join(dir, '.git'))) return dir;
		const parent = dirname(dir);
		if (parent === dir) return null; // filesystem root
		dir = parent;
	}
}

/** The filesystem root above `start` (`/` on posix). The format-root fallback
 * outside a git repo, so the `.formatignore` walk spans the whole path and the
 * cwd never enters. Mirrors the native `filesystem_root`. */
function filesystem_root(start) {
	let dir = start;
	for (;;) {
		const parent = dirname(dir);
		if (parent === dir) return dir;
		dir = parent;
	}
}

/** `abs` relative to `format_root` as a `/`-joined string (empty for
 * `format_root` itself). Returns null only in the degenerate case where `abs`
 * is not under `format_root`, which the boundary resolution never produces (the
 * format root is always an ancestor-or-self of the root). Mirrors the native
 * `path_to_rel` over a `strip_prefix`. */
function rel_under(format_root, abs) {
	const rel = path_relative(format_root, abs);
	if (rel === '') return '';
	if (rel === '..' || rel.startsWith(`..${sep}`) || isAbsolute(rel)) return null;
	return rel
		.split(/[/\\]/)
		.filter((s) => s !== '' && s !== '.')
		.join('/');
}

/** Directories from `format_root` (inclusive) down to `leaf` (inclusive),
 * shallowest first. Mirrors the native `ancestor_chain`. */
function ancestor_chain(format_root, leaf) {
	const chain = [];
	let dir = leaf;
	for (;;) {
		chain.push(dir);
		if (dir === format_root) break;
		const parent = dirname(dir);
		if (parent === dir) break;
		dir = parent;
	}
	chain.reverse();
	return chain;
}

/**
 * Expand files and directories into a sorted, deduplicated list of files to
 * format, mirroring the native `discover_files`: root args are validated
 * upfront (any bad one fails the run with exit 2 — one that resolves to neither
 * a file nor a directory, or a *file* whose extension tsv doesn't format),
 * explicit files are always included regardless of the ignore files (the caller
 * named them) but not regardless of extension, and directories recurse with the
 * extension filter. Symlinks inside directories are not followed. Traversal
 * errors below a valid root are non-fatal and returned for reporting. See
 * `collect_root` for the gitignore-aware ignore semantics.
 */
function discover_files(paths) {
	const stats = paths.map((path) => {
		try {
			return statSync(path);
		} catch {
			return null;
		}
	});
	// Both argument errors, reported together. The extension check applies only to
	// *file* args (a directory is a scope, filtered by the walk) and comes from
	// `tsv_discover` via the WASM binding, message and all, so this never
	// hand-mirrors the native extension list. The receiver is unused there, so a
	// throwaway stack is the whole cost of reaching it.
	const arg_policy = new IgnoreStack();
	const bad = paths
		.map((path, i) => {
			if (stats[i]?.isDirectory()) return undefined;
			if (stats[i]?.isFile()) return arg_policy.unsupported_extension_error(path);
			return `${path}: not a file or directory`;
		})
		.filter((message) => message !== undefined);
	if (bad.length > 0) {
		for (const message of bad) eprint(`error: ${message}\n`);
		process.exit(2);
	}

	// canonical cwd so it compares cleanly with canonicalized roots below
	let cwd;
	try {
		cwd = realpathSync(process.cwd());
	} catch {
		cwd = process.cwd();
	}

	let files = [];
	const errors = [];
	const warnings = [];
	for (let i = 0; i < paths.length; i++) {
		if (stats[i].isFile()) {
			// explicit file bypasses the ignore files (not the extension check,
			// which the validation above already applied)
			files.push(paths[i]);
		} else {
			collect_root(paths[i], cwd, files, errors, warnings);
		}
	}
	files.sort(compare_paths);
	files = files.filter((path, i) => path !== files[i - 1]);
	if (paths.length > 1) {
		const seen = new Set();
		files = files.filter((path) => {
			let canonical;
			try {
				canonical = realpathSync(path);
			} catch {
				canonical = path;
			}
			if (seen.has(canonical)) return false;
			seen.add(canonical);
			return true;
		});
	}
	errors.sort();
	// dedupe so a directory pruned via two overlapping roots warns at most once
	warnings.sort();
	const deduped_warnings = warnings.filter((w, i) => w !== warnings[i - 1]);
	return { files, errors, warnings: deduped_warnings };
}

/**
 * Set up the ignore evaluation for one directory `root`, then recurse. Inside a
 * git repo the format root is the repo root (a hard stop — nothing above it is
 * read, so `--check` is reproducible); outside one it's the filesystem root (so
 * an ancestor `.formatignore` is honored). Preloads the `IgnoreStack` for the
 * ancestor chain from there down: `.formatignore` at each level (and, inside a
 * repo, a `.prettierignore` it shadows per-directory), and `.gitignore` at each
 * level when in a repo. Mirrors the native `collect_root`.
 */
function collect_root(root, cwd, files, errors, warnings) {
	let root_abs;
	try {
		root_abs = realpathSync(root);
	} catch {
		root_abs = isAbsolute(root) ? root : join(cwd, root);
	}
	const repo_root = find_repo_root(root_abs);
	const in_repo = repo_root !== null;
	const format_root = repo_root ?? filesystem_root(root_abs);

	const stack = new IgnoreStack();
	let heuristic_active = true;
	// `root` relative to the format root (always an ancestor-or-self of it, so
	// never null); '' means `root` *is* the format root
	const base_rel = rel_under(format_root, root_abs) ?? '';

	// preload the ancestors *above* `root` (format root → `root`'s parent). `root`
	// and everything below reads its own ignore files in collect_recursive from
	// the listing it already fetches, so an ignore-file-free subtree costs no
	// speculative opens; `root` is excluded here to avoid reading its ignores
	// twice. Ancestors above `root` aren't listed, so they keep the direct open.
	for (const ancestor of ancestor_chain(format_root, root_abs).slice(0, -1)) {
		const anchor = rel_under(format_root, ancestor) ?? '';
		// tsv layer: `.formatignore` everywhere; inside a repo, a `.prettierignore`
		// it shadows per-directory (hierarchical, like `.formatignore` — deeper
		// wins). No listing for ancestors, so the read outcome stands in for
		// presence: a present-but-unreadable `.formatignore` (`unreadable`, warned)
		// shadows `.prettierignore` just as in the listed case — only a genuinely
		// `absent` one falls through.
		let tsv = null;
		if (in_repo) {
			const fr = read_ignore_file(join(ancestor, FORMATIGNORE_FILE), warnings);
			if (fr.kind === 'content') {
				tsv = fr.content;
			} else if (fr.kind === 'absent') {
				const pr = read_ignore_file(join(ancestor, PRETTIERIGNORE_FILE), warnings);
				if (pr.kind === 'content') tsv = pr.content;
			}
		} else {
			const fr = read_ignore_file(join(ancestor, FORMATIGNORE_FILE), warnings);
			if (fr.kind === 'content') tsv = fr.content;
		}
		if (tsv !== null) stack.push_tsv(anchor, tsv);
		// `.gitignore` layer: only inside a git repo
		if (in_repo) {
			const gr = read_ignore_file(join(ancestor, GITIGNORE_FILE), warnings);
			if (gr.kind === 'content') {
				stack.push_gitignore(anchor, gr.content);
				heuristic_active = false;
			}
		}
	}

	// The recursion uses the leaf-only matcher query (is_ignored_leaf, inside
	// classify_dir/should_format_file), exact only when an entry's ancestors are
	// already cleared — true for everything the walk descends into, but not for
	// `root` itself when it's under an ignored ancestor (e.g. `tsv format
	// build/sub` with a gitignored `build/`). Gate it once with the full
	// ancestor-walking is_ignored: an ignored root means nothing under it is in
	// scope. Mirrors the native collect_root.
	if (base_rel !== '' && stack.is_ignored(base_rel, true)) return;

	collect_recursive(
		root,
		base_rel,
		true,
		in_repo,
		stack,
		heuristic_active,
		files,
		errors,
		warnings
	);
}

/** A path's components, split on either separator spelling where the platform
 * has two. `\` is a legal filename byte on posix, so it counts only on Windows
 * — mirroring Rust's `is_sep_byte`. */
function split_path_components(path) {
	return sep === '\\' ? path.split(/[/\\]/) : path.split('/');
}

/** Does `path` already end in a separator? Same platform rule as
 * `split_path_components` — the `PathBuf::push` test for whether a separator
 * must be inserted. */
function ends_with_sep(path) {
	return path.endsWith('/') || (sep === '\\' && path.endsWith('\\'));
}

/** Component-wise path ordering matching Rust's `PathBuf` ordering — a
 * separator splits components and a shorter prefix sorts first, so `a/y.ts`
 * precedes `a-b/x.ts` (plain string order would invert them: `-` < `/`). The
 * parity claim is scoped to ASCII/BMP names: JS compares UTF-16 code units
 * while Rust compares UTF-8 bytes, so astral-plane names (≥ U+10000) order
 * differently. A Windows path's drive prefix rides along as one leading
 * component where Rust splits it into `Prefix` + `RootDir`, which reaches the
 * same verdict for every pair sharing a root. */
function compare_paths(a, b) {
	if (a === b) return 0;
	const as = split_path_components(a);
	const bs = split_path_components(b);
	const len = Math.min(as.length, bs.length);
	for (let i = 0; i < len; i++) {
		if (as[i] !== bs[i]) return as[i] < bs[i] ? -1 : 1;
	}
	return as.length - bs.length;
}

function collect_recursive(
	dir,
	dir_rel,
	is_target_root,
	in_repo,
	stack,
	heuristic_active,
	files,
	errors,
	warnings
) {
	let entries;
	try {
		entries = readdirSync(dir, { withFileTypes: true });
	} catch (error) {
		errors.push(`${dir}: read_dir failed: ${error.message}`);
		return;
	}
	// Single pass over the listing for the ignore-file presence flags this dir
	// needs, rather than a scan per name; `readdir` order is arbitrary so there's
	// nothing to short-circuit on. An ignore file's content is still opened only
	// when present (below). Mirrors the native collect_recursive.
	let has_formatignore = false;
	let has_prettierignore = false;
	let has_gitignore = false;
	for (const e of entries) {
		if (e.name === FORMATIGNORE_FILE) has_formatignore = true;
		else if (e.name === PRETTIERIGNORE_FILE) has_prettierignore = true;
		else if (in_repo && e.name === GITIGNORE_FILE) has_gitignore = true;
	}
	// tsv layer: `.formatignore` whenever present (every level, in or out of a
	// repo); inside a repo, a `.prettierignore` is the drop-in fallback at every
	// level (hierarchical, like `.formatignore`), used solely when no *sibling*
	// `.formatignore` is present. Precedence is by presence (the listing), not
	// readability — a present-but-unreadable `.formatignore` still shadows:
	// read_ignore_file warns and yields no rules.
	let tsv = null;
	if (has_formatignore) {
		const r = read_ignore_file(join(dir, FORMATIGNORE_FILE), warnings);
		if (r.kind === 'content') tsv = r.content;
	} else if (in_repo && has_prettierignore) {
		const r = read_ignore_file(join(dir, PRETTIERIGNORE_FILE), warnings);
		if (r.kind === 'content') tsv = r.content;
	}
	let tsv_pushed = false;
	if (tsv !== null) {
		stack.push_tsv(dir_rel, tsv);
		tsv_pushed = true;
	}
	// inside a repo, a sibling `.formatignore` shadows this dir's `.prettierignore`
	// (one tsv layer per directory) — its rules go unread here. Warn (decision +
	// text from Rust, single source of truth with the native CLI), pointing at
	// merging the patterns into `.formatignore`. Fires at every directory (a shadow
	// is per-directory, not target-root like the outside-repo warning below).
	{
		const warning = stack.prettierignore_shadowed_warning(
			dir,
			in_repo,
			has_prettierignore,
			has_formatignore
		);
		if (warning != null) warnings.push(warning);
	}
	// outside a git repo a target-root `.prettierignore` is silently skipped (tsv
	// reads `.formatignore` there) — warn (decision + text from Rust, single source
	// of truth with the native CLI), pointing at the rename / `git init` fixes.
	if (is_target_root) {
		const warning = stack.prettierignore_outside_repo_warning(
			dir,
			in_repo,
			has_prettierignore,
			has_formatignore
		);
		if (warning != null) warnings.push(warning);
	}
	// `.gitignore` layer: only inside a repo (has_gitignore implies in_repo); turns
	// the heuristic off for children. A present-but-unreadable `.gitignore` warns
	// and is not pushed — so the heuristic stays on, which the warning makes visible.
	let git_pushed = false;
	if (has_gitignore) {
		const r = read_ignore_file(join(dir, GITIGNORE_FILE), warnings);
		if (r.kind === 'content') {
			stack.push_gitignore(dir_rel, r.content);
			git_pushed = true;
		}
	}
	const child_heuristic = heuristic_active && !git_pushed;

	for (const entry of entries) {
		// PathBuf::push parity: insert the platform separator, and only when the
		// dir doesn't already end with one — so a trailing-slash root (`tsv
		// format src/`) yields `src/a.ts`, not `src//a.ts`, on either platform.
		// The separator has to be `sep` rather than a hardcoded `/`: discovered
		// paths are what the CLI prints and hands to the formatter, and the
		// native CLI's `entry.path()` spells them natively, so a `/` here makes
		// the two CLIs disagree on Windows over the same tree.
		const path = ends_with_sep(dir) ? `${dir}${entry.name}` : `${dir}${sep}${entry.name}`;
		// `path` relative to the format root, for matching ('' = the format root)
		const child_rel = dir_rel === '' ? entry.name : `${dir_rel}/${entry.name}`;
		if (entry.isDirectory()) {
			// the per-directory prune/descend decision — safety nets, the
			// build-output heuristic (+ its shadow warning), and the matcher —
			// lives in `tsv_discover`, shared with the native CLI via the verdict.
			// The FS walk + layer push/pop stay here.
			const verdict = stack.classify_dir(entry.name, child_rel, child_heuristic);
			if (verdict !== 'descend') {
				// on `prune_warn` fetch the message from Rust (single source of
				// truth — the JS CLI never templates it). One warning per pruned dir.
				if (verdict === 'prune_warn') warnings.push(stack.heuristic_shadow_warning(child_rel));
				continue;
			}
			// the child reads its own ignore files when we recurse into it
			collect_recursive(
				path,
				child_rel,
				false,
				in_repo,
				stack,
				child_heuristic,
				files,
				errors,
				warnings
			);
		} else if (entry.isFile() && stack.should_format_file(entry.name, child_rel)) {
			files.push(path);
		}
	}

	if (git_pushed) stack.pop_gitignore();
	if (tsv_pushed) stack.pop_tsv();
}

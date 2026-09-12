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
 * explicit `--jobs N` is honored up to the native CLI's ceiling of 4x the
 * logical CPUs (`clamp_worker_count`, restated here so both surfaces refuse
 * the same numbers), announced when it bites. Under it, the count is clamped
 * to the file count, and a worker the host refuses narrows the pool (see
 * `format_files_parallel`) rather than failing the run. `--jobs 1`,
 * `--content`/`--stdin`, and `--list` stay on one thread either way.
 * Which engine a worker binds is decided by whether the main thread's
 * `./index.js` exposes a `wasm_module`: the WASM package hands the compiled
 * module across and the worker initializes from it (compiled code is shared
 * process-wide, so no worker recompiles), while the N-API package has no such
 * export and its workers just load the addon. That same discriminant sizes the
 * pool — both the threshold and the worker count are per engine, because a WASM
 * run shares its cores with V8's wasm tier-up and a native one doesn't.
 * A WASM trap (deeply nested input) is contained to its file on either path:
 * `format_one` reinstantiates the engine and carries on (see
 * `recover_engine_suffix`).
 *
 * Exit codes: `format` — 0 clean, 1 would-change (`--check`), 2 errors;
 * `parse` — 0 ok, 1 error. Flag-parsing errors exit 1 (both commands); `format`'s
 * post-parse usage/validation errors (an invalid `--source-type`, conflicting inputs) exit 2.
 */

import { Buffer } from 'node:buffer';
import {
	existsSync,
	lstatSync,
	readdirSync,
	readFileSync,
	readSync,
	realpathSync,
	statSync,
	writeFileSync,
	writeSync
} from 'node:fs';
import { availableParallelism } from 'node:os';
import { dirname, isAbsolute, join, relative as path_relative, resolve, sep } from 'node:path';
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

/** Workers per logical CPU an explicit `--jobs` is held to — the native CLI's
 * ceiling (`MAX_WORKERS_PER_LOGICAL_CPU` in `tsv_cli`'s `cli/stack.rs`),
 * restated here by hand like the pool's warning strings; the package test
 * reads both spellings and fails on drift, so the two surfaces refuse the
 * same numbers. Declared with the other pool constants, above the `main()`
 * call — a `const` below it is dead-zoned for the whole run. */
const MAX_WORKERS_PER_LOGICAL_CPU = 4;

/** The stack, in MiB, every pool worker reserves — the native CLI's `STACK_SIZE`
 * (`tsv_cli`'s `cli/stack.rs`), restated by hand like the ceiling above and gated against
 * it by the package test. A V8 worker takes Node's `resourceLimits.stackSizeMb` (4 MiB
 * unless set) where the main thread has ~1 MiB, so a deep file formatted on one route
 * and overflowed on the other, and which route ran depended on how many OTHER files the
 * tree held. Reserving the native size in every worker, and retrying a main-thread
 * overflow in one (`retry_overflowed_files`), gives every route the worker's ceiling —
 * on the WASM engine, the module's own stack. The reservation is committed lazily, so it
 * costs address space and ~no memory. Bun and Deno ignore the option. */
const WORKER_STACK_SIZE_MB = 32;

/** Valid `--parser` values (shared by `format` and `parse` — `FORMATTERS` and
 * `PARSERS` are keyed by the same names). */
const PARSER_NAMES = new Set(['svelte', 'typescript', 'css']);

/** Whether a WASM engine reinstantiation already failed on this thread — one
 * warning per thread, not one per remaining file (each worker is its own module
 * instance with its own copy, so a pool of N can warn N times; the recovery
 * itself is per-instance too, so the scope is the right one). Declared with the
 * other module state above the `main()` call: a `let` below it is dead-zoned for
 * the whole run. */
let engine_recovery_failed = false;

/**
 * Recover the WASM engine after a trap, returning the suffix `format_one`
 * appends to that file's error message.
 *
 * A trap poisons the whole instance (the shadow-stack pointer is a mutable
 * global the trap never restores), so without recovery every later file on this
 * thread reports a bogus `memory access out of bounds`. `reinstantiate` — the
 * WASM packages' trap-recovery hook, present on both the auto-init entry and
 * the worker entry — swaps in a fresh instance from the already-compiled
 * module (no recompile), and every already-bound export follows it. The native
 * package exports no such hook and needs none: it runs no instance to poison
 * (its overflow is a process-fatal SIGSEGV), so there the suffix is empty and
 * a `WebAssembly.RuntimeError` cannot arise. `cause` is the error that stranded
 * the instance — a trap, or the `RangeError` V8 raises when a deep call exhausts
 * the engine's native stack first — and only words the suffix. The discovery
 * `IgnoreStack`s are freed deterministically (see `discover_files`/`collect_root`),
 * so no stale wasm-backed handle survives into the fresh instance.
 */
function recover_engine_suffix(cause) {
	if (engine.reinstantiate === undefined) return '';
	const trapped = cause instanceof WebAssembly.RuntimeError;
	if (!engine_recovery_failed) {
		try {
			engine.reinstantiate();
			return trapped
				? ' (WASM engine trapped and was reinstantiated)'
				: ' (WASM engine reinstantiated)';
		} catch (error) {
			engine_recovery_failed = true;
			eprint(
				`warning: could not reinstantiate the WASM engine after ${trapped ? 'a trap' : 'a RangeError'} (${error.message}); remaining files on this thread may fail\n`
			);
		}
	}
	return trapped
		? ' (WASM engine trapped; reinstantiation failed)'
		: ' (WASM engine reinstantiation failed)';
}

/**
 * Free a wasm-backed `IgnoreStack` deterministically, so its bytes return to the
 * engine now rather than at GC (and a handle outliving a post-trap
 * `reinstantiate` is leaked by the registry's guard rather than freed). Engine-split: the native addon's
 * `IgnoreStack` is a napi-rs class with no `free()` — napi-rs owns its
 * lifetime, and there is no instance to poison there — so absence is a no-op,
 * not an error.
 */
function free_ignore_stack(stack) {
	if (typeof stack.free === 'function') stack.free();
}

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

const FORMAT_HELP = `Usage: tsv format [<paths...>] [--check] [--list] [--content <s> | --stdin] [--parser <p>] [--source-type <t>]

Format source code in place (near-Prettier output).

Paths are formatted in place (written only when the output differs) and
changed paths print to stdout; directories recurse over
.ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css, honoring .gitignore
(hierarchically, in a git tree) plus hierarchical .formatignore /
.prettierignore. A named file or directory is bounded by the ignore
files alone: one they exclude is skipped (quietly for a file a
.formatignore or .prettierignore excludes, with a warning otherwise),
and a named file's extension must still be one tsv formats.
--content/--stdin print formatted source to stdout.

Options:
  --content <s>     content to format, printed to stdout (requires --parser)
  --stdin           read from stdin, print to stdout (requires --parser)
  --parser <p>      parser type: svelte | typescript | css (--content/--stdin only)
  --source-type <t> TypeScript parse goal: script | module (default: module, retried as a script; --content/--stdin only; an error with svelte/css)
  --check           check instead of writing/printing: exit 1 if any input would change
  --list            list the discovered in-scope files (one per line) without formatting; path mode only
  --jobs <n>        worker thread count (default: scaled to this machine and engine; explicit values capped at 4x logical)

Exit codes: 0 clean, 1 would change (--check), 2 errors.
`;

const PARSE_HELP = `Usage: tsv parse [<file>] [--pretty] [--content <s> | --stdin] [--parser <p>] [--source-type <t>] [--no-locations]

Parse source code into AST JSON.

Options:
  --pretty          pretty-print JSON output
  --content <s>     content to parse (requires --parser)
  --stdin           read from stdin (requires --parser)
  --parser <p>      parser type: svelte | typescript | css
  --source-type <t> TypeScript parse goal: script | module (default: module; an error with svelte/css)
  --no-locations    omit per-node loc (span-only wire; svelte also omits name_loc; no-op for css)
`;

/**
 * Each command's argh grammar, as `parse_argv` reads it: every flag — a switch, or a
 * value-taking option whose `parse(value)` returns `{value}` or `{error}` — how many
 * positionals the command takes, its subcommands, and the help it prints. A flag's
 * key in the parsed `values` is its name in snake_case, the field argh derives the
 * flag from. Declared above the top-level `await`, for the reason `WRITE_BACKOFF` is.
 */
const FORMAT_ARGS = {
	options: {
		'--content': {},
		'--stdin': { switch: true },
		'--parser': { parse: parser_type_from_str },
		'--source-type': {},
		'--check': { switch: true },
		'--list': { switch: true },
		'--jobs': { parse: usize_from_str }
	},
	positionals: Infinity,
	help: FORMAT_HELP
};

const PARSE_ARGS = {
	options: {
		'--pretty': { switch: true },
		'--content': {},
		'--stdin': { switch: true },
		'--parser': { parse: parser_type_from_str },
		'--source-type': {},
		'--no-locations': { switch: true }
	},
	positionals: 1,
	help: PARSE_HELP
};

const TOP_LEVEL_ARGS = {
	options: { '--version': { switch: true } },
	positionals: 0,
	subcommands: { format: FORMAT_ARGS, parse: PARSE_ARGS },
	help: HELP
};

/**
 * `Atomics.wait`'s timer cell — the one way to sleep synchronously, which is
 * what `write_fd`'s EAGAIN retry needs. Never notified, so every wait runs its
 * full timeout.
 *
 * ⚠️ Declared HERE, above the top-level `await` below, and not beside
 * `write_fd` where it belongs: module evaluation stops at that await, so every
 * `const`/`let` further down is still in its temporal dead zone for the whole
 * of `main()`. Function declarations hoist and are fine anywhere; module STATE
 * is not, and the failure is a `ReferenceError` from whichever call happens to
 * need it first.
 */
const WRITE_BACKOFF = new Int32Array(new SharedArrayBuffer(4));

/**
 * The strict UTF-8 decoder every byte read takes — one spelling of the parity
 * contract with Rust's `read_to_string` (`decode_source` says why), shared by the
 * source reads and the ignore-file reads. `fatal` refuses invalid bytes instead of
 * substituting U+FFFD; `ignoreBOM: true` *keeps* a leading BOM (the option name is
 * inverted), as `read_to_string` does. A decoder built without `{stream: true}` is
 * stateless across `decode` calls, so one instance serves the whole run — and it is
 * declared here, above the top-level `await`, for the reason `WRITE_BACKOFF` is.
 */
const UTF8_STRICT = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

// the two roles this file plays: the CLI itself, and the worker it spawns for
// path-mode formatting (see `format_files_parallel`)
if (isMainThread) {
	await main();
} else {
	run_format_worker();
}

async function main() {
	const { values, subcommand } = parse_argv(process.argv.slice(2), TOP_LEVEL_ARGS);
	// the native `TopLevel::run` order: the version switch first, then the subcommand
	if (values.version) {
		print_version();
		return;
	}
	switch (subcommand?.name) {
		case 'format':
			await run_format(subcommand);
			break;
		case 'parse':
			run_parse(subcommand);
			break;
		default:
			// no subcommand: argh's required-subcommand refusal, as `TopLevel::run` spells it
			exit_with_error(
				1,
				'One of the following subcommands must be present:\n    help\n    parse\n    format\n\nRun tsv --help for more information.'
			);
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

/**
 * Parse argv by argh's grammar — a transcription of argh 0.1's `parse_struct_args`,
 * the loop the native CLI runs, because every word the two parsers read differently
 * is an exit-code or message split between the bins:
 *
 * - `help` / `--help` ahead of `--` sets a flag and parsing CONTINUES: a later
 *   `-`-prefixed word refuses (`Trailing arguments are not allowed after `help``),
 *   an earlier bad value or a later extra positional still errors, and the help
 *   prints only once the whole argv has parsed (`tsv format help` never formats a
 *   directory named `help`);
 * - a word naming a subcommand hands it the rest of argv and ends this level — with
 *   `help` prepended when a help word came first, as argh's `prepend_help` does, so
 *   `tsv help format` is `tsv format help` — whether or not a `--` came before it;
 * - a value-taking flag takes the next word verbatim (`--content --check` formats the
 *   text `--check`), refuses a second occurrence (`duplicate values provided`), and
 *   has its value parsed on the spot, so a bad value errors in argh's words and in
 *   argv order;
 * - there are no inline values (`--flag=value`) and no short flags: any other
 *   `-`-prefixed word is unrecognized — looked up as an own key, so an
 *   `Object.prototype` name (`--constructor`) is unrecognized too;
 * - a positional past the command's last is unrecognized where it appears.
 *
 * Returns `{values, positionals}`, or `{values, subcommand: {name, values,
 * positionals}}` once a subcommand took the rest. Every refusal exits 1 in the
 * exact text the native `main` prints (`exit_argh`).
 */
function parse_argv(args, spec) {
	const values = {};
	const positionals = [];
	let help = false;
	let options_ended = false;
	for (let i = 0; i < args.length; i++) {
		const arg = args[i];
		if (!options_ended && (arg === 'help' || arg === '--help')) {
			help = true;
			continue;
		}
		if (!options_ended && arg.startsWith('-')) {
			if (arg === '--') {
				options_ended = true;
				continue;
			}
			if (help) exit_argh('Trailing arguments are not allowed after `help`.');
			const option = Object.hasOwn(spec.options, arg) ? spec.options[arg] : undefined;
			if (option === undefined) exit_argh(`Unrecognized argument: ${arg}\n`);
			const key = arg.slice(2).replaceAll('-', '_');
			if (option.switch) {
				values[key] = true;
				continue;
			}
			if (i + 1 >= args.length) exit_argh(`No value provided for option '${arg}'.\n`);
			const value = args[++i];
			const parsed = Object.hasOwn(values, key)
				? { error: 'duplicate values provided' }
				: (option.parse?.(value) ?? { value });
			if (parsed.error !== undefined) {
				exit_argh(`Error parsing option '${arg}' with value '${value}': ${parsed.error}\n`);
			}
			values[key] = parsed.value;
			continue;
		}
		const subcommand = spec.subcommands?.[arg];
		if (subcommand !== undefined && Object.hasOwn(spec.subcommands, arg)) {
			const rest = args.slice(i + 1);
			const parsed = parse_argv(help ? ['help', ...rest] : rest, subcommand);
			return { values, subcommand: { name: arg, ...parsed } };
		}
		if (positionals.length >= spec.positionals) exit_argh(`Unrecognized argument: ${arg}\n`);
		positionals.push(arg);
	}
	if (help) {
		print(spec.help);
		process.exit(0);
	}
	return { values, positionals };
}

/** `ParserType`'s `FromStr`, as argh applies it to a `--parser` value: the parser
 * (`ts` is an accepted alias), or the native error text. */
function parser_type_from_str(name) {
	const resolved = name === 'ts' ? 'typescript' : name;
	return PARSER_NAMES.has(resolved)
		? { value: resolved }
		: { error: `Unknown parser type: '${name}'. Valid types: svelte, typescript, css` };
}

/**
 * Rust's `usize::from_str`, as argh applies it to a `--jobs` value: the value as a
 * BigInt, since the bound sits past Number's safe range, or `ParseIntError`'s own text. The accepted set is ASCII digits
 * with an OPTIONAL LEADING `+`, refused above `usize::MAX` — both edges restated,
 * since a value one bin calls an error and the other silently clamps is exactly the
 * drift this hand-mirroring exists to prevent. The bound is the 64-bit `usize` every
 * published platform triple has; a 32-bit native target would refuse lower, and this
 * would have to ask.
 */
function usize_from_str(text) {
	if (text === '') return { error: 'cannot parse integer from empty string' };
	const digits = text.startsWith('+') ? text.slice(1) : text;
	if (!/^\d+$/.test(digits)) return { error: 'invalid digit found in string' };
	const value = BigInt(digits);
	if (value > 18446744073709551615n) return { error: 'number too large to fit in target type' };
	return { value };
}

/** Validate a `--source-type` value (the TypeScript goal axis), or exit `code` —
 * the native CLI validates it after argh, in the command (`parse_source_type_arg`),
 * so a bad value exits with the command's usage code (`parse` 1, `format` 2). Absent →
 * `undefined` (the `module` default); the source type only affects the TypeScript
 * parser. */
function resolve_source_type(source_type, code) {
	if (source_type === undefined || source_type === 'module' || source_type === 'script') {
		return source_type;
	}
	eprint(`Error: invalid --source-type '${source_type}' (expected 'script' or 'module')\n`);
	process.exit(code);
}

/** Refuse a `--source-type` on a language that has no goal axis, or exit `code` —
 * the native CLI's `check_source_type_language`, word for word. Svelte hard-wires
 * `Module` and css has no goal, so a caller naming one there asked for something
 * that cannot be honored and must be told; every binding takes that stance
 * (`tsv_wasm`'s `read_options` throws on a set key), so this bin is not the one
 * surface that drops the flag silently. Called once the parser is resolved and
 * before the parse/format call, so `source_type` is thereafter `undefined` on
 * every goalless language and one options bag still serves whichever engine. */
function refuse_source_type_language(parser, source_type, code) {
	if (source_type !== undefined && parser !== 'typescript') {
		eprint(
			`Error: --source-type is only supported for typescript (the ${parser} parser has no source type)\n`
		);
		process.exit(code);
	}
}

/**
 * Call `ask` with a throwaway `IgnoreStack`, freeing it on the spot.
 *
 * The argument refusals below are `tsv_discover`'s, taken through the binding so this
 * file never hand-mirrors their text — and they ride the `IgnoreStack` class (the
 * receiver is unused) because a class is what the package facade re-exports. Both come
 * BEFORE any walk has a stack to ask, so each needs one of its own; the `finally` is not
 * optional, since no wasm-backed handle may outlive discovery into a run that can
 * `reinstantiate`.
 */
function with_arg_policy(ask) {
	const arg_policy = new IgnoreStack();
	try {
		return ask(arg_policy);
	} finally {
		free_ignore_stack(arg_policy);
	}
}

/** The refusal for a file whose extension tsv does not handle, or `undefined` for one it
 * does — the binding's `tsv_discover::unsupported_extension_error`, message and all, so
 * this never hand-mirrors the native extension list. */
function unsupported_extension_error(path) {
	return with_arg_policy((policy) => policy.unsupported_extension_error(path));
}

/** The traversal error for a relative root the working directory cannot resolve — the
 * binding's `tsv_discover::unresolvable_root_error`. */
function unresolvable_root_error(root) {
	return with_arg_policy((policy) => policy.unresolvable_root_error(root));
}

/** Extension-based parser detection, mirroring the native `ParserType::from_extension`. */
function parser_from_extension(path) {
	if (path.endsWith('.svelte')) return 'svelte';
	if (path.endsWith('.css')) return 'css';
	return 'typescript';
}

/**
 * The source type a path's own EXTENSION settles, or `undefined` when it settles
 * none — restated by hand from the native `tsv_ts::Goal::from_extension` (as
 * `clamp_worker_count` is), so both `tsv` bins format a path under the same grammar.
 *
 * `.mjs` and `.mts` are ES modules whatever any config says, so the module-then-script
 * fallback has nothing to fall back to there: it exists to reach a legacy sloppy
 * script, which a file that is a module by name cannot be. Every other extension stays
 * `undefined` and takes the fallback. No source the module grammar accepts is affected
 * — the retry fires only on a module parse failure.
 */
function source_type_from_extension(path) {
	return path.endsWith('.mjs') || path.endsWith('.mts') ? 'module' : undefined;
}

async function run_format({ values, positionals }) {
	if (values.content !== undefined || values.stdin) {
		format_single(values, positionals);
	} else {
		await format_paths(values, positionals);
	}
}

/** `--content`/`--stdin` mode — format one input to stdout (or `--check` it). */
function format_single(values, positionals) {
	if (positionals.length > 0) {
		exit_with_error(2, 'Error: --content/--stdin cannot be combined with file paths');
	}
	if (values.jobs !== undefined) {
		exit_with_error(
			2,
			'Error: --jobs applies to file paths; --content/--stdin format a single input'
		);
	}
	if (values.list) {
		exit_with_error(
			2,
			'Error: --list applies to file paths; --content/--stdin format a single input'
		);
	}
	// --source-type is content/stdin-only and TypeScript-only; a bad value is a
	// usage error (exit 2). Checked AFTER the paths/--jobs/--list refusals, in the
	// native CLI's order, so a doubly-bad invocation names the same error on both
	// bins. Path mode rejects --source-type in format_paths.
	const source_type = resolve_source_type(values.source_type, 2);
	const flag = values.content !== undefined ? '--content' : '--stdin';
	if (values.parser === undefined) {
		exit_with_error(2, `Error: ${flag} requires --parser <svelte|typescript|css>`);
	}
	refuse_source_type_language(values.parser, source_type, 2);
	const input = values.content !== undefined ? values.content : read_stdin(2);
	let formatted;
	try {
		// one bag, handed to whichever formatter — the refusal above leaves
		// `source_type` undefined on the goalless languages, which reads as the default
		formatted = FORMATTERS[values.parser](input, { sourceType: source_type });
	} catch (error) {
		exit_with_error(2, `Parse error: ${error.message}`);
	}
	if (values.check) {
		if (formatted !== input) {
			exit_with_error(1, 'would change');
		}
	} else {
		print(formatted);
	}
}

/** Path mode — discover files, format (in parallel above
 * `WORKER_FILE_THRESHOLD`), report in sorted order. */
async function format_paths(values, positionals) {
	if (positionals.length === 0) {
		exit_with_error(2, 'Error: No input provided. Use a file path, --content, or --stdin');
	}
	if (values.parser !== undefined) {
		exit_with_error(
			2,
			'Error: --parser applies to --content/--stdin; file paths use extension detection'
		);
	}
	// Path mode resolves the source type per file instead of taking one for the
	// whole run: a Svelte or CSS file on the same command line has no source type to
	// honor, and every JS/TS file formats under whichever grammar accepts it unless
	// its own extension settles one (`source_type_from_extension`). Mirrors the
	// native CLI's refusal, word for word.
	if (values.source_type !== undefined) {
		exit_with_error(
			2,
			'Error: --source-type applies to --content/--stdin; file paths take the module grammar, retried as a script'
		);
	}
	if (values.list && values.check) {
		exit_with_error(2, 'Error: --list and --check cannot be combined');
	}
	// An explicit `--jobs` is held to the machine's ceiling AHEAD of discovery, as the
	// native CLI sizes its pool before it walks (`run_paths` in `commands/format.rs`):
	// the clamp's warning then precedes the discovery diagnostics on both bins, and it
	// fires on an empty scope too. `--list` sizes no pool and warns nothing, on both.
	const explicit_jobs =
		values.list || values.jobs === undefined ? undefined : clamp_worker_count(values.jobs);

	const {
		files,
		errors: traversal_errors,
		warnings,
		all_arguments_excluded
	} = discover_files(positionals);
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
		// One write for the whole listing, as the native CLI does (`run_paths` in
		// `commands/format.rs`): a per-path write re-enters the writer and issues a
		// syscall for each of (potentially thousands of) lines. It also decides what a
		// closed reader costs — `write_fd` goes quiet on `EPIPE` by *catching* it, so a
		// per-path loop pays a throw and a catch once per remaining path, where one
		// write pays one.
		print(join_lines(files));
		if (traversal_errors.length > 0) process.exit(2);
		return;
	}
	// An empty run every argument accounts for — each a named file an ignore rule
	// excluded — is not the error: what a pre-commit hook hands over when only ignored
	// files are staged. An excluded directory argument keeps the error. Mirrors the native
	// `exit_if_nothing_in_scope`.
	if (files.length === 0 && traversal_errors.length === 0 && !all_arguments_excluded) {
		// neutral wording: an empty result can mean "no
		// .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css here" *or* "all of them are
		// ignored" (e.g. a target under a gitignored dir), so don't imply a
		// wrong-extension cause. `--list` reports the empty set and exits 0.
		exit_with_error(
			2,
			'Error: No files to format — no unignored .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css files in scope'
		);
	}

	const jobs = resolve_jobs(explicit_jobs, files.length);
	const outcomes =
		jobs > 1
			? await format_files_parallel(files, values.check, jobs)
			: await retry_overflowed_files(files, format_files(files, values.check), values.check);

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
	const changed_paths = [];
	for (let i = 0; i < files.length; i++) {
		const outcome = outcomes[i];
		if (outcome.kind === 'unchanged') {
			unchanged++;
		} else if (outcome.kind === 'changed') {
			changed++;
			changed_paths.push(files[i]);
		} else {
			errors++;
			eprint(`error: ${files[i]}: ${outcome.message}\n`);
		}
	}
	print(join_lines(changed_paths));

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
		source = decode_source(readFileSync(path));
	} catch (error) {
		return { kind: 'error', message: `read failed: ${error.message}` };
	}
	let formatted;
	const parser = parser_from_extension(path);
	try {
		formatted = FORMATTERS[parser](source, {
			// TypeScript only; the other two formatters reject a SET key, and the
			// goalless extensions resolve to `undefined`, which reads as the default
			sourceType: parser === 'typescript' ? source_type_from_extension(path) : undefined
		});
	} catch (error) {
		// A trap (`WebAssembly.RuntimeError` — a parse error is a plain `Error`)
		// is not a per-file failure: a stack overflow (input nested past ~2,500
		// levels — generated or minified code) leaves `__stack_pointer` where the
		// deep call left it, poisoning the instance so every later file throws
		// `memory access out of bounds` too. That held on BOTH paths — a worker's
		// catch here kept its poisoned instance claiming files, so the parallel
		// run was only ever saved by whichever files the healthy workers drained
		// first. So recover before the next file: `reinstantiate` (WASM engine
		// only; see `recover_engine_suffix`) swaps in a fresh instance from the
		// already-compiled module, and only this file reports an error.
		// A `RangeError` takes the same recovery: V8's own stack overflow strands the
		// instance too (see `recover_engine_suffix`), and any other one costs at most a
		// reinstantiate it did not need.
		if (error instanceof WebAssembly.RuntimeError || error instanceof RangeError) {
			return {
				kind: 'error',
				message: `${error.message}${recover_engine_suffix(error)}`,
				// V8's own stack ran out before the module's: what a pool worker's larger
				// stack may still clear (`retry_overflowed_files`)
				native_stack_overflow: error instanceof RangeError
			};
		}
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

/**
 * Re-run, in a one-worker pool, each file whose format on this thread ran out of V8's own
 * native stack, and put the retried outcomes in place of the failed ones — `outcomes` is
 * indexed by `files`, and is returned.
 *
 * The main thread has ~1 MiB of V8 stack and a pool worker `WORKER_STACK_SIZE_MB`, so a
 * deep file's verdict used to turn on which route the file count picked. A worker reaches
 * the WASM module's own ceiling, whose trap is the same answer on every route. It costs
 * one worker start, paid only when a file overflowed. Deno ignores the stack option, and
 * there the retry reports the error again; Bun ignores it too, but its retry runs wasm
 * code the failed attempt has already warmed, which recursion reaches deeper on (docs/cli.md
 * §Recursion Depth). The failed attempt wrote nothing, so no file is written twice.
 */
async function retry_overflowed_files(files, outcomes, check) {
	const retry = [];
	for (let i = 0; i < outcomes.length; i++) {
		if (outcomes[i].native_stack_overflow) retry.push(i);
	}
	if (retry.length === 0) return outcomes;
	const retried = await format_files_parallel(
		retry.map((i) => files[i]),
		check,
		1
	);
	for (let j = 0; j < retry.length; j++) outcomes[retry[j]] = retried[j];
	return outcomes;
}

/**
 * The process's cgroup CPU quota in whole cores, rounded down — `Infinity` when none
 * is set or none can be read. A transcription of the quota Rust's
 * `available_parallelism` applies on Linux (std's `cgroups::quota`), which the native
 * CLI's `--jobs` ceiling and default width both read, while Node's and Bun's
 * `availableParallelism()` count the affinity mask alone: under a `--cpus` or
 * `CPUQuota=` limit the two bins would otherwise name different ceilings, and this one
 * would size its pool past the quota. (Deno's count already applies it, and a second
 * `min` changes nothing.)
 *
 * The process's place in the hierarchy comes from `/proc/self/cgroup`, where a v1
 * entry naming the `cpu` controller wins over the v2 one. The quota is the smallest
 * `limit / period` on the path from that cgroup up to its mount: `cpu.max` under v2
 * (read only when the path holds `cgroup.controllers`, so it really is cgroup2), and
 * `cpu.cfs_quota_us` over `cpu.cfs_period_us` under v1. An unlimited quota (`max`,
 * `-1`) is not a count and sets nothing, and a failed read reads as no quota, so this
 * can only ever lower a count.
 */
function cgroup_cpu_quota() {
	let entries;
	try {
		entries = readFileSync('/proc/self/cgroup', 'utf-8');
	} catch {
		return Infinity; // not Linux, or no procfs
	}
	let group = null;
	for (const line of entries.split('\n')) {
		const fields = line.split(':');
		if (fields.length < 3) continue;
		// the second field lists a v1 hierarchy's controllers, and is empty for v2
		const version = fields[1] === '' ? 2 : fields[1].split(',').includes('cpu') ? 1 : 0;
		// a v1 `cpu` entry already found wins, since it names its controllers
		if (version === 0 || (group !== null && version === 2)) continue;
		// the path, without its leading `/`
		group = { version, path: fields.slice(2).join(':').slice(1) };
	}
	if (group === null) return Infinity;
	return group.version === 2 ? cgroup_v2_quota(group.path) : cgroup_v1_quota(group.path);
}

/** The smallest cgroup v2 `cpu.max` quota from `group` up to the cgroup2 mount at its
 * standard location (file-hierarchy(7); std reads no other). */
function cgroup_v2_quota(group) {
	const mount = '/sys/fs/cgroup';
	const dir = join(mount, group);
	if (!existsSync(join(dir, 'cgroup.controllers'))) return Infinity; // not cgroup2
	return smallest_quota_upward(dir, mount, (level) => {
		const [limit, period] = readFileSync(join(level, 'cpu.max'), 'utf-8').split('\n')[0].split(' ');
		return cgroup_quota_cores(limit, period);
	});
}

/** The smallest cgroup v1 CFS quota from `group` up to its `cpu` controller's mount —
 * the first of `cgroup_v1_mounts` that holds the group. */
function cgroup_v1_quota(group) {
	for (const [mount, path] of cgroup_v1_mounts(group)) {
		const dir = join(mount, path);
		if (!existsSync(dir)) continue; // guessed the mount wrong
		return smallest_quota_upward(dir, mount, (level) =>
			cgroup_quota_cores(
				readFileSync(join(level, 'cpu.cfs_quota_us'), 'utf-8').trim(),
				readFileSync(join(level, 'cpu.cfs_period_us'), 'utf-8').trim()
			)
		);
	}
	return Infinity;
}

/** The smallest quota `read_level` reads from `dir` up to `mount` inclusive, one level at
 * a time — the shape both of std's cgroup walks take. A level whose files cannot be read
 * sets nothing (a cgroup2 root has no `cpu.max`). */
function smallest_quota_upward(dir, mount, read_level) {
	let quota = Infinity;
	for (let level = dir; level === mount || level.startsWith(`${mount}/`); level = dirname(level)) {
		try {
			quota = Math.min(quota, read_level(level));
		} catch {
			// nothing readable at this level
		}
	}
	return quota;
}

/** Where a cgroup v1 `cpu` controller may be mounted, each paired with `group`
 * re-rooted under it: the two paths cgroups(7) names, then the first matching
 * `/proc/self/mountinfo` entry — read only once both guesses miss, and trimmed for a
 * bind mount of a subtree. */
function* cgroup_v1_mounts(group) {
	yield ['/sys/fs/cgroup/cpu', group];
	yield ['/sys/fs/cgroup/cpu,cpuacct', group];
	let mountinfo;
	try {
		mountinfo = readFileSync('/proc/self/mountinfo', 'utf-8');
	} catch {
		return;
	}
	for (const line of mountinfo.split('\n')) {
		// `id parent major:minor root mount-point options… - fstype source super-options`
		const items = line.trim().split(' ');
		if (items.length < 7) continue;
		const [root, mount_point] = [items[3], items[4]];
		if (items.at(-3) !== 'cgroup' || !items.at(-1).split(',').includes('cpu')) continue;
		if (!root.startsWith('/')) return;
		const sub = root.slice(1);
		// a bind mount whose bound subtree does not hold this process's cgroup
		if (sub !== '' && group !== sub && !group.startsWith(`${sub}/`)) continue;
		yield [mount_point, group.slice(sub.length).replace(/^\//, '')];
		return;
	}
}

/** `limit / period` in whole cores, or `Infinity` when either is not a count (an
 * unlimited `max` or `-1`) or the period is zero. */
function cgroup_quota_cores(limit, period) {
	if (!/^\d+$/.test(limit ?? '') || !/^\d+$/.test(period ?? '')) return Infinity;
	return Number(period) > 0 ? Math.floor(Number(limit) / Number(period)) : Infinity;
}

/** Logical and physical CPU counts. The logical count is the one the native CLI
 * reads — the affinity mask capped by the cgroup CPU quota (`cgroup_cpu_quota`,
 * floored at one core as std floors it). The SMT width is one read of
 * `thread_siblings_list`, not one per CPU; anywhere that read fails — every
 * non-Linux platform included — it reads 1 and physical degrades to logical,
 * so the topology can only ever lower a worker count, never raise it. */
function cpu_topology() {
	let logical;
	try {
		logical = Math.min(availableParallelism(), Math.max(1, cgroup_cpu_quota()));
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
 * How many workers to actually spawn: 1 means "stay on this thread". `explicit` is
 * the `--jobs` value already held to the machine's ceiling (`clamp_worker_count`),
 * or `undefined` when the flag was not given.
 *
 * `WORKER_FILE_THRESHOLD` governs the **default** only. An explicit `--jobs N`
 * bypasses it at any file count — the user asked, and an explicit width is also
 * the only way to compare the parallel and sequential paths at a fixed size,
 * which is what calibrating the threshold and `default_jobs` took (every size
 * that calibration uses sits far under the ceiling `clamp_worker_count` holds
 * the flag to).
 *
 * Both forms clamp to the file count (a worker with no file to claim is pure
 * startup cost) and to a floor of 1, so `--jobs 0` means the same as `--jobs 1`
 * — matching the native clamp, which `--jobs 0` had escaped on its streaming
 * path (`format_streamed`).
 */
function resolve_jobs(explicit, file_count) {
	if (explicit === undefined) {
		return file_count < WORKER_FILE_THRESHOLD ? 1 : clamp_jobs(default_jobs(), file_count);
	}
	return clamp_jobs(explicit, file_count);
}

/** A worker count clamped to what there is work for, floored at 1. */
function clamp_jobs(jobs, file_count) {
	return Math.max(1, Math.min(jobs, file_count));
}

/**
 * Hold an explicit `--jobs` to `MAX_WORKERS_PER_LOGICAL_CPU` per logical CPU,
 * mirroring the native `clamp_worker_count` — same ceiling, same message, said
 * out loud when it bites and never silently.
 *
 * The native ceiling is about address space and task slots; neither is what
 * bites here. A worker is a whole V8 isolate — ~13 MB resident on either
 * engine, where the native worker's reservation is lazily committed and costs
 * ~none — so an unbounded width walks up the machine's *memory* instead, and
 * the file-count clamp is no bound on exactly the trees large enough to
 * matter: measured on a 12-logical-CPU / 16 GB machine, a 1,300-file tree at
 * full width peaked at 787 live threads and 5.2 GB for a run the default
 * width finishes 29x sooner. Past the wall the end state is the kernel's OOM
 * kill — a SIGKILL with zero output that no catch survives — where a thread
 * the OS refuses (EAGAIN) throws and narrows the pool gracefully. That
 * narrowing stays; this bound exists for the wall that refuses nothing until
 * it kills.
 */
function clamp_worker_count(requested) {
	// `requested` is `usize_from_str`'s BigInt, compared and printed as one: the warning
	// restates what argh prints — the parsed `usize`, so `+5` and `005` read `5` — and a
	// Number rounds past 2^53, where `--jobs 18446744073709551615` would print `…552000`.
	const ceiling = cpu_topology().logical * MAX_WORKERS_PER_LOGICAL_CPU;
	if (requested > BigInt(ceiling)) {
		eprint(`warning: --jobs ${requested} exceeds this machine's ceiling; using ${ceiling}\n`);
		return ceiling;
	}
	return Number(requested);
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
 * A pool that cannot be brought up at all falls back to this thread — whether
 * `new Worker` refused, or every worker died before claiming a file (a torn
 * install whose worker-side import or init rejects: construction succeeds and
 * the death arrives as an `error` event). A worker that dies mid-run does not:
 * every index left unfilled surfaces as an error, because turning one worker's
 * death into a silent "unchanged" would report a clean run over files nobody
 * formatted. A dying worker still posts what it finished (see
 * `run_format_worker`), so "unfilled" means genuinely unformatted rather than
 * "claimed by the worker that died".
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
			workers.push(
				new Worker(new URL(import.meta.url), {
					workerData: worker_data,
					resourceLimits: { stackSizeMb: WORKER_STACK_SIZE_MB }
				})
			);
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

	const worker_errors = [];
	await Promise.all(
		workers.map(
			(worker) =>
				new Promise((resolve) => {
					worker.on('message', (results) => {
						for (const { index, outcome } of results) outcomes[index] = outcome;
					});
					worker.on('error', (error) => {
						// Held rather than printed, because whether a death is an error
						// depends on what the pool did: if no worker claimed a file this
						// thread formats everything and the deaths are the reason its
						// warning gives, and otherwise each is printed below as the cause
						// of the sweep. Not counted either way — the sweep turns every
						// index nobody reported into its own error, which is the
						// accounting the summary line reports, and a worker that throws
						// still posts what it finished, so the sweep lands only on
						// genuinely unformatted files.
						worker_errors.push(error.message);
						resolve();
					});
					worker.on('exit', () => resolve());
				})
		)
	);

	// The pool came up and then every worker died before claiming anything — the
	// cursor never moved, so no file was touched. This thread is the floor under the
	// pool here as it is under a refused spawn (the native `spawn_pool`'s callers
	// drain on the calling thread when it comes up empty); the sweep below would
	// instead report every file as unformatted and exit 2 having done no work. A
	// cursor that moved means a worker may have written, and then the sweep is the
	// honest answer.
	if (Atomics.load(cursor, 0) === 0) {
		// one reason per distinct death, as the native `could not start format workers`
		// warning carries its error — a pool of identical failures says it once
		const why = worker_errors.length > 0 ? ` (${[...new Set(worker_errors)].join('; ')})` : '';
		eprint(`warning: no format worker ran${why}; formatting on one thread\n`);
		return format_files(files, check);
	}
	for (const message of worker_errors) eprint(`error: format worker failed: ${message}\n`);

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
 * on Node and Bun; the package test pins the ordering by spawning this file as
 * a worker with a cursor `Atomics.add` rejects). A termination this `finally`
 * never runs — a heap OOM, a kill — still loses the batch, and there the
 * conservative over-report is the right answer anyway. (A Node worker carries its
 * own old-gen cap, 2048 MiB, whether or not `resourceLimits` is passed — the pool
 * passes `stackSizeMb` alone and Node fills the rest with those same defaults, so
 * it neither sets nor moves the heap limit — and exceeding it arrives as an
 * `ERR_WORKER_OUT_OF_MEMORY` `error` event rather than killing the process. The
 * parent survives it; the batch does not, since the worker is terminated before
 * the `finally` can post.)
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

function run_parse({ values, positionals }) {
	const flag_parser = values.parser;
	// --source-type is validated upfront (exit 1) like the native CLI; it only
	// affects the TypeScript parser (svelte is always a module, css has no goal).
	const source_type = resolve_source_type(values.source_type, 1);

	// Input precedence mirrors the native `InputArgs::resolve`: --content > --stdin > file.
	let input;
	let parser;
	if (values.content !== undefined) {
		if (flag_parser === undefined) {
			exit_with_error(1, 'Error: --content requires --parser <svelte|typescript|css>');
		}
		parser = flag_parser;
		input = values.content;
	} else if (values.stdin) {
		if (flag_parser === undefined) {
			exit_with_error(1, 'Error: --stdin requires --parser <svelte|typescript|css>');
		}
		parser = flag_parser;
		input = read_stdin(1);
	} else if (positionals.length > 0) {
		const path = positionals[0];
		if (flag_parser === undefined) {
			// The path's extension picks the parser, so it must be one tsv handles: the
			// dispatch has no unknown arm, and a `.md` would otherwise parse as
			// TypeScript and report a syntax error about prose. Same check, same message
			// as `format <file>` and the native `InputArgs::resolve`; an explicit
			// `--parser` is the override.
			const unsupported = unsupported_extension_error(path);
			if (unsupported !== undefined) {
				exit_with_error(1, `Error: ${unsupported}`);
			}
			parser = parser_from_extension(path);
		} else {
			parser = flag_parser;
		}
		try {
			input = decode_source(readFileSync(path));
		} catch (error) {
			exit_with_error(1, `Error: Error reading file '${path}': ${error.message}`);
		}
	} else {
		exit_with_error(1, 'Error: No input provided. Use a file path, --content, or --stdin');
	}

	refuse_source_type_language(parser, source_type, 1);

	// --no-locations drops per-node `loc` (span-only wire; svelte also `name_loc`,
	// a no-op for css); orthogonal to --source-type (the source type drives the TS
	// parser, no-locations the writer), so they compose. `locations` is a parse-only
	// option — format emits no wire and rejects the key.
	const no_locations = values.no_locations === true;
	let json;
	try {
		json = PARSERS[parser](input, {
			locations: !no_locations,
			sourceType: source_type
		});
	} catch (error) {
		exit_with_error(1, `Parse error: ${error.message}`);
	}
	if (values.pretty) {
		// A re-serialization, where the native CLI re-indents the compact bytes
		// without ever reading them back — and byte-identical to it all the same: the
		// wire writer spells every number in ECMAScript's own `Number::toString` form
		// (so a `JSON.parse`/`stringify` round trip changes no token), emits no
		// integer-like keys (so V8's key reordering never fires), and escapes exactly
		// what `JSON.stringify` escapes. The package test pins the equality on both
		// bins; the compact default is the verbatim wire string from Rust either way.
		json = JSON.stringify(JSON.parse(json), null, '\t');
	}
	print(`${json}\n`);
}

/**
 * Write to a fd synchronously, surviving a **non-blocking pipe**.
 *
 * Sync is the requirement: `process.stdout.write` is async on pipes, so a
 * `process.exit` right after it truncates output. But a bare
 * `writeFileSync(fd, text)` is only safe while the fd stays BLOCKING, and this
 * CLI takes that away from itself — spawning the worker pool initializes the
 * parent's `process.stdout`/`stderr` (the workers' stdio is piped through it),
 * which flips the fd to non-blocking. A full pipe then makes the very next
 * write throw `EAGAIN`, which nothing caught: `tsv format .` into `head`,
 * `less`, `grep` or a CI log collector died with a Node stack trace and a
 * half-written path, AFTER having already rewritten files on disk — so the
 * report of what changed was lost while the changes were not. Only large trees
 * hit it, because only they spawn workers and only they fill a pipe.
 *
 * So: loop over `writeSync`, honor partial writes, sleep 1 ms and retry on
 * `EAGAIN` (a spin would burn a core against a consumer as slow as a human
 * scrolling `less`), and go quiet on `EPIPE` — the consumer closed, which is
 * what `| head` is, and the conventional answer there is to stop, not to
 * throw.
 */
function write_fd(fd, text) {
	const buf = Buffer.from(text, 'utf-8');
	let offset = 0;
	while (offset < buf.length) {
		try {
			offset += writeSync(fd, buf, offset);
		} catch (error) {
			if (error.code === 'EAGAIN') {
				Atomics.wait(WRITE_BACKOFF, 0, 0, 1);
				continue;
			}
			if (error.code === 'EPIPE') return;
			throw error;
		}
	}
}

/** Synchronous stdout write. */
function print(text) {
	write_fd(1, text);
}

/** Synchronous stderr write. */
function eprint(text) {
	write_fd(2, text);
}

/** Print `message` as one stderr line and exit with `code` — the shape every
 * argument refusal and single-input failure takes, mirroring the native
 * `cli::out::exit_with_error`. The message is printed verbatim (the caller spells
 * its own `Error:` / `Parse error:` prefix, as the native command does). */
function exit_with_error(code, message) {
	eprint(`${message}\n`);
	process.exit(code);
}

/** An argh early exit as the native `main` prints it: argh's own output, the `Run tsv
 * --help` pointer, exit 1. */
function exit_argh(output) {
	exit_with_error(1, `${output}\nRun tsv --help for more information.`);
}

/** Read all of stdin, exiting with the calling command's error code on
 * failure (`format` uses 2, `parse` uses 1 — mirroring the native CLI). */
function read_stdin(exit_code) {
	try {
		return decode_source(read_fd_to_end(0));
	} catch (error) {
		exit_with_error(exit_code, `Error: Error reading from stdin: ${error.message}`);
	}
}

/** Read `fd` to EOF, waiting out `EAGAIN` as `write_fd` does. Whether fd 0 blocks
 * belongs to the open file description this process shares with its parent, and a
 * Node parent that opens its own piped `process.stdin` flips it to non-blocking
 * under the child — where `readFileSync(0)` throws the moment the pipe is
 * momentarily empty, reporting a slow writer as a read error. The same rule the
 * native `Input::from_stdin` holds. */
function read_fd_to_end(fd) {
	const chunks = [];
	const chunk = Buffer.allocUnsafe(64 * 1024);
	for (;;) {
		let n;
		try {
			n = readSync(fd, chunk, 0, chunk.length, null);
		} catch (error) {
			if (error.code === 'EAGAIN') {
				Atomics.wait(WRITE_BACKOFF, 0, 0, 1);
				continue;
			}
			if (error.code === 'EOF') break;
			throw error;
		}
		if (n === 0) break;
		chunks.push(Buffer.from(chunk.subarray(0, n)));
	}
	return Buffer.concat(chunks);
}

/**
 * Decode source bytes as **strict** UTF-8, mirroring Rust's `read_to_string`
 * — which is what the native CLI reads every file and stdin with, and which
 * REFUSES invalid bytes rather than repairing them.
 *
 * Node's `readFileSync(path, 'utf-8')` does the opposite: it substitutes
 * U+FFFD for every invalid sequence and hands back a string that looks fine.
 * On the format path that is not a wrong message, it is DATA LOSS — a `.ts`
 * file holding one stray byte inside a string literal still parses after the
 * substitution, so the formatter writes the repaired text back over the
 * author's file and reports `1 formatted`, exit 0, where the native CLI
 * refuses and leaves the bytes alone. `read_ignore_file` below has always
 * decoded strictly for the same reason; the source path is where it costs
 * more.
 *
 * The decoder is `UTF8_STRICT` (declared with the module state, which is why it
 * keeps a BOM). The thrown message is Rust's own wording, so every caller's
 * existing `${error.message}` interpolation reproduces the native text byte for
 * byte.
 */
function decode_source(buf) {
	try {
		return UTF8_STRICT.decode(buf);
	} catch {
		throw new Error('stream did not contain valid UTF-8');
	}
}

/** Read an ignore file, classifying the outcome so the walk can surface a
 * silently-dropped file and keep precedence by presence. Returns `{kind:
 * 'content', content}` on success, `{kind: 'absent'}` for ENOENT (missing, or
 * raced away after the listing — silent), or `{kind: 'unreadable'}` for any other
 * failure, pushing a non-fatal warning. **Strict UTF-8** (`decode_source`) to match
 * Rust's `read_to_string` — Node's `readFileSync(path, 'utf-8')` would lossily
 * replace invalid bytes, silently applying a mangled ignore file the native CLI
 * drops — and, for invalid UTF-8, the same failure text, so that warning reads
 * identically on both bins (the native one interpolates the `read_to_string` error,
 * whose wording `decode_source` reproduces). Any OTHER read failure carries each
 * runtime's own wording (`EACCES: permission denied, open '…'` here, `Permission
 * denied (os error 13)` natively), as the traversal errors do — the parity
 * contract covers the decision, not those texts. Mirrors the native
 * `read_ignore_file`. */
function read_ignore_file(path, warnings) {
	try {
		return { kind: 'content', content: decode_source(readFileSync(path)) };
	} catch (error) {
		if (error.code === 'ENOENT') return { kind: 'absent' };
		warnings.push(`could not read ${path} (${error.message}); its ignore rules are not applied`);
		return { kind: 'unreadable' };
	}
}

/** Whether `path` names an ignore file: a regular file, reached through a symlink
 * the way reading it is — a directory of that name holds no rules (reading it would
 * fail with EISDIR and warn about rules that were never there), and a dangling link
 * is absent. The one presence rule both walks apply, mirroring the native
 * `is_ignore_file`: the ancestor preload probes with it, and the descent asks it of
 * a listing entry that is a symlink. */
function is_ignore_file(path) {
	try {
		return statSync(path).isFile();
	} catch {
		return false;
	}
}

/** How an in-tree `.gitignore` is present, by git's rule rather than `is_ignore_file`'s:
 * `'symlink'` for a symbolic link — git does not follow one in a working tree and applies
 * none of its rules, so the walk warns and treats it as a file whose rules could not be
 * read — `'file'` for a regular file, and `'absent'` otherwise. Asked without following
 * the link; a listing's own entry type answers the same question for the descent. Mirrors
 * the native `GitignorePresence`. */
function gitignore_presence(path) {
	try {
		const stat = lstatSync(path);
		if (stat.isSymbolicLink()) return 'symlink';
		return stat.isFile() ? 'file' : 'absent';
	} catch {
		return 'absent';
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
	// `path.relative` hands back a normalized path — no `.` or empty component — so
	// the split alone spells it; the split is the platform's (`\` is a filename byte on
	// posix, and a file named `a\b.ts` must reach the matcher as one segment, as it does
	// on the native CLI). `path_components` is the other normalizer here, for ORDER,
	// and keeps a head `.` this must not
	return split_path_components(rel).join('/');
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
 * a named file or directory is bounded by the ignore files alone (one a rule excludes
 * is skipped, with the warning `excluded_argument_warning` calls for, and
 * `all_arguments_excluded` says whether every argument was such a file), a named file is
 * held to its extension first, and directories recurse with the extension filter.
 * Symlinks inside directories are not followed. Traversal errors below a valid root
 * are non-fatal and returned for reporting. See `collect_root` for the
 * gitignore-aware ignore semantics.
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
	// *file* args (a directory is a scope, filtered by the walk).
	const bad = paths
		.map((path, i) => {
			if (stats[i]?.isDirectory()) return undefined;
			if (stats[i]?.isFile()) return unsupported_extension_error(path);
			return `${path}: not a file or directory`;
		})
		.filter((message) => message !== undefined);
	if (bad.length > 0) {
		for (const message of bad) eprint(`error: ${message}\n`);
		process.exit(2);
	}

	// canonical cwd so it compares cleanly with canonicalized roots below; null when it
	// cannot be resolved (a deleted working directory, where `process.cwd()` itself
	// throws), which only a relative root that fails to canonicalize ever asks for.
	// Mirrors the native `walk_args`.
	let cwd = null;
	try {
		cwd = realpathSync(process.cwd());
	} catch {
		// left null — see collect_root
	}

	let files = [];
	const errors = [];
	const warnings = [];
	// every file argument is graded in one scope, moved from each one's directory to the
	// next's rather than rebuilt per directory. Mirrors the native `walk_args`.
	const file_scope = new_file_scope();
	let excluded_files = 0;
	for (let i = 0; i < paths.length; i++) {
		if (stats[i].isFile()) {
			// the extension check is the validation's above, already applied
			if (collect_file(paths[i], cwd, file_scope, files, errors, warnings)) {
				excluded_files++;
			}
		} else {
			collect_root(paths[i], cwd, files, errors, warnings);
		}
	}
	free_file_scope(file_scope);
	files.sort(compare_paths);
	files = files.filter((path, i) => path !== files[i - 1]);
	if (roots_can_overlap(paths, stats)) {
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
	// both channels sorted and deduped, as the native `discover_into` does: overlapping
	// roots re-walk the shared subtree, so the same unreadable path or pruned directory
	// would otherwise report twice — and count twice in the summary's error total
	return {
		files,
		errors: sort_dedup(errors),
		warnings: sort_dedup(warnings),
		all_arguments_excluded: paths.length > 0 && excluded_files === paths.length
	};
}

/** Whether two of `paths` can yield the same file, which is when the canonical dedup
 * above must run — mirroring the native `roots_can_overlap`: a file argument among
 * them (it can repeat, or sit under a directory root), or one canonical directory
 * root an ancestor-or-self of another. Disjoint roots can't share a file (the walk
 * follows no symlink), so `tsv format src lib` pays one `realpathSync` per root, not
 * per file. `stats` is the argument check's one classification; a `realpathSync`
 * failure after it is a race and reads as "may overlap". */
function roots_can_overlap(paths, stats) {
	if (paths.length < 2) return false;
	if (stats.some((stat) => stat.isFile())) return true;
	const roots = [];
	for (const path of paths) {
		try {
			roots.push(realpathSync(path));
		} catch {
			return true;
		}
	}
	return roots.some((a, i) => roots.some((b, j) => i !== j && rel_under(a, b) !== null));
}

/** `lines` sorted with exact duplicates removed (the strings are byte-identical only
 * for the same underlying failure, so this collapses repeats without hiding a
 * distinct one). */
function sort_dedup(lines) {
	lines.sort();
	return lines.filter((line, i) => line !== lines[i - 1]);
}

/** `paths` as one newline-terminated block — the shape both bulk stdout writes
 * take, so a listing and a changed-path report cannot drift apart. */
function join_lines(paths) {
	let block = '';
	for (const path of paths) block += `${path}\n`;
	return block;
}

/**
 * The tsv layer one directory contributes, read by the precedence both walks
 * share: `.formatignore` whenever present (every level, in or out of a repo);
 * inside a repo, a `.prettierignore` is the drop-in fallback at every level
 * (hierarchical, like `.formatignore`), used solely when no *sibling*
 * `.formatignore` is present. Precedence is by PRESENCE, not readability — a
 * present-but-unreadable `.formatignore` still shadows: read_ignore_file warns and
 * yields no rules rather than silently falling through to `.prettierignore`.
 *
 * A shadowed `.prettierignore` is warned about here too (decision + text from
 * Rust, single source of truth with the native CLI), at whichever directory the
 * shadow sits — the ancestor preload and the descent both reach this, so the
 * warning fires whether the shadowing directory is walked or only preloaded. One
 * statement of the ladder, mirroring the native `tsv_layer_content`; the two
 * callers differ only in how presence was learned (a listing, or an `is_ignore_file` probe).
 *
 * `dir` is the directory's ABSOLUTE path — an ancestor as preloaded, a walked
 * directory as `collect_recursive`'s `dir_abs` — never an argument spelling: the one
 * spelling its ignore files are read by and its diagnostics name it by, whichever root
 * or spelling reached it, so overlapping roots (`tsv format . sub`, `tsv format . ./`)
 * report one warning rather than one per spelling. Mirrors the native `tsv_layer_content`.
 * @returns {{content: string, prettierignore: boolean} | null} the layer's content and
 *   whether it was read from `.prettierignore`, or null for no layer
 */
function tsv_layer_content(dir, has_formatignore, has_prettierignore, in_repo, stack, warnings) {
	const warning = stack.prettierignore_shadowed_warning(
		dir,
		in_repo,
		has_prettierignore,
		has_formatignore
	);
	if (warning != null) warnings.push(warning);
	let prettierignore;
	if (has_formatignore) prettierignore = false;
	else if (in_repo && has_prettierignore) prettierignore = true;
	else return null;
	const r = read_ignore_file(
		join(dir, prettierignore ? PRETTIERIGNORE_FILE : FORMATIGNORE_FILE),
		warnings
	);
	return r.kind === 'content' ? { content: r.content, prettierignore } : null;
}

/** Push a `tsv_layer_content` layer onto `stack` at `anchor`, as the file it was read
 * from — the file a warning about one of its rules names. Mirrors the native
 * `TsvLayer::push_onto`. */
function push_tsv_layer(stack, anchor, layer) {
	if (layer.prettierignore) stack.push_prettierignore(anchor, layer.content);
	else stack.push_formatignore(anchor, layer.content);
}

/**
 * Load `stack` with the ignore layers of `dirs` — a directory root's ancestors, from
 * `format_root` down to its parent (`collect_root`), reading nothing inside one a rule
 * excludes (`push_dir_layers`), which puts the root out of scope — returning whether the
 * build-output heuristic is still on below them (no `.gitignore` among them was read).
 * Mirrors the native `preload_ancestors`.
 */
function preload_ancestors(stack, dirs, format_root, in_repo, warnings) {
	let heuristic_active = true;
	for (const dir of dirs) {
		if (push_dir_layers(stack, dir, format_root, in_repo, warnings)?.gitignore) {
			heuristic_active = false;
		}
	}
	return heuristic_active;
}

/**
 * Push the ignore layers of `dir` — a directory no listing is held for — onto `stack`,
 * anchored relative to `format_root`, returning which layers it pushed, or `null`,
 * reading nothing, when a rule in the layers already pushed excludes `dir`, itself or
 * through an ancestor: the walk prunes such a directory without listing it, so it never
 * reads — nor warns about — the ignore files inside, and a named file's quiet skip
 * depends on that. The one preload a named path gets, for a directory root's ancestors
 * (`preload_ancestors`) and for each directory a file argument's scope moves into
 * (`enter_file_scope`); `stack` must already hold what this pushed for every ancestor of
 * `dir`. Mirrors the native `push_dir_layers`.
 *
 * The gate is all this adds to `push_layers`, which the descent reaches too: the preload
 * differs from a walked directory only in PROBING for the files a listing would have
 * named (`probe_ignore_presence`).
 * @returns {{tsv: boolean, gitignore: boolean} | null}
 */
function push_dir_layers(stack, dir, format_root, in_repo, warnings) {
	const anchor = rel_under(format_root, dir) ?? '';
	if (stack.is_ignored(anchor, true)) return null;
	return push_layers(stack, dir, anchor, in_repo, probe_ignore_presence(dir, in_repo), warnings);
}

/**
 * Which of one directory's ignore files are there, probed a file at a time — what a
 * directory no listing is held for costs. `.formatignore` and `.prettierignore` take the
 * descent's own rule (is_ignore_file), `.gitignore` git's (gitignore_presence).
 *
 * Outside a repo `.prettierignore` is left false unprobed: nothing reads it there but the
 * target root's heads-up, which only the descent raises — and outside a repo the format
 * root is the FILESYSTEM root, so an ancestor chain is long enough that a stat per level
 * is worth not paying. Mirrors the native `IgnorePresence::probe`.
 * @returns {{formatignore: boolean, prettierignore: boolean, gitignore: string}}
 */
function probe_ignore_presence(dir, in_repo) {
	return {
		formatignore: is_ignore_file(join(dir, FORMATIGNORE_FILE)),
		prettierignore: in_repo && is_ignore_file(join(dir, PRETTIERIGNORE_FILE)),
		gitignore: in_repo ? gitignore_presence(join(dir, GITIGNORE_FILE)) : 'absent'
	};
}

/**
 * Push one directory's ignore layers onto `stack` at `anchor`, returning which it pushed
 * — the single statement of the ladder both walks run, so a preload and a descent cannot
 * drift on it.
 *
 * The tsv layer first (tsv_layer_content, which also warns about a `.prettierignore` its
 * sibling shadows), then the `.gitignore`, which pushes only from a regular file: a
 * symlinked one git does not follow and an unreadable one has no rules to apply, so both
 * warn and push nothing — leaving the build-output heuristic on for the subtree, which
 * the warning is what makes visible. Nothing here is gated on `in_repo` beyond what
 * tsv_layer_content gates itself: outside a repo `presence.gitignore` is already
 * `'absent'`, by the rule each caller read it with.
 *
 * `dir` is the directory's ABSOLUTE path — the one spelling its ignore files are read by
 * and every ignore-file diagnostic names it by, whichever root or argument spelling
 * reached it (see tsv_layer_content). Mirrors the native `push_layers`.
 * @returns {{tsv: boolean, gitignore: boolean}}
 */
function push_layers(stack, dir, anchor, in_repo, presence, warnings) {
	const pushed = { tsv: false, gitignore: false };
	const layer = tsv_layer_content(
		dir,
		presence.formatignore,
		presence.prettierignore,
		in_repo,
		stack,
		warnings
	);
	if (layer !== null) {
		push_tsv_layer(stack, anchor, layer);
		pushed.tsv = true;
	}
	// the path is built only in the two arms that need it — most directories have no
	// `.gitignore`, and outside a repo none is ever read
	if (presence.gitignore === 'symlink') {
		warnings.push(stack.gitignore_symlink_warning(join(dir, GITIGNORE_FILE)));
	} else if (presence.gitignore === 'file') {
		const gr = read_ignore_file(join(dir, GITIGNORE_FILE), warnings);
		if (gr.kind === 'content') {
			stack.push_gitignore(anchor, gr.content);
			pushed.gitignore = true;
		}
	}
	return pushed;
}

/** Take back off `stack` what one `push_layers` put on it — a traversal unwinding out of
 * a directory, or a file scope popping back to a shallower one. `pushed` may be the
 * `null` `push_dir_layers` returns for a directory a rule excludes, which pushed nothing.
 * Mirrors the native `PushedLayers::pop_from`. */
function pop_layers(stack, pushed) {
	if (pushed?.tsv) stack.pop_tsv();
	if (pushed?.gitignore) stack.pop_gitignore();
}

/**
 * A named path made absolute: canonicalized, or — when it will not canonicalize —
 * resolved lexically as the native `absolutize` does (`resolve` settles `.` and `..`, and
 * asks the process for no cwd: an absolute path needs none, and a relative one is joined
 * onto the `cwd` already held). `null` is a relative path with no working directory to
 * join it onto, which the caller refuses (`unresolvable_root_error`): taken as named, it
 * would anchor on no format root and read none of its ancestors' ignore files. Mirrors
 * the native `absolute_named_path`.
 */
function absolute_named_path(path, cwd) {
	try {
		return realpathSync(path);
	} catch {
		if (isAbsolute(path)) return resolve(path);
		return cwd === null ? null : resolve(cwd, path);
	}
}

/**
 * The format root above `dir` and whether it is a repo root: inside a git repo the repo
 * root (a hard stop — nothing above it is read), outside one the filesystem root (so an
 * ancestor `.formatignore` is honored). Mirrors the native `format_root_of`.
 */
function format_root_of(dir) {
	const repo_root = find_repo_root(dir);
	return repo_root === null
		? { format_root: filesystem_root(dir), in_repo: false }
		: { format_root: repo_root, in_repo: true };
}

/**
 * The ignore scope file arguments are graded in: the layers from a format root down
 * through the directory the latest file argument sat in. Each argument moves it to its
 * own directory (`enter_file_scope`) — popping back to the two directories' common
 * ancestor and pushing down — so an ignore file above many named files is read and parsed
 * once, and none inside a directory a rule excludes is read at all (`push_dir_layers`,
 * whose `null` a directory's `pushed` holds then, as does every one below it). The caller
 * frees it (`free_file_scope`). Mirrors the native `FileScope`.
 */
function new_file_scope() {
	return { format_root: null, in_repo: false, stack: null, dirs: [] };
}

/** Move `scope` to `dir`, a file argument's canonical directory. Mirrors the native
 * `FileScope::enter`. */
function enter_file_scope(scope, dir, warnings) {
	if (scope.dirs.length > 0 && scope.dirs[scope.dirs.length - 1].dir === dir) return;
	const { format_root, in_repo } = format_root_of(dir);
	if (format_root !== scope.format_root || in_repo !== scope.in_repo) {
		free_file_scope(scope);
		scope.format_root = format_root;
		scope.in_repo = in_repo;
		scope.stack = new IgnoreStack();
		scope.dirs = [];
	}
	const chain = ancestor_chain(format_root, dir);
	let shared = 0;
	while (
		shared < scope.dirs.length &&
		shared < chain.length &&
		scope.dirs[shared].dir === chain[shared]
	) {
		shared++;
	}
	while (scope.dirs.length > shared) {
		pop_layers(scope.stack, scope.dirs.pop().pushed);
	}
	for (const level of chain.slice(shared)) {
		scope.dirs.push({
			dir: level,
			pushed: push_dir_layers(scope.stack, level, format_root, in_repo, warnings)
		});
	}
}

/** Free `scope`'s stack, if it has one. */
function free_file_scope(scope) {
	if (scope.stack !== null) free_ignore_stack(scope.stack);
	scope.stack = null;
}

/**
 * One file argument: into `files` unless an ignore rule excludes it — through an ancestor
 * directory or at the file itself, as it would exclude the file from the walk that
 * reached it — in which case it is skipped, with the warning the rule's file calls for
 * (`excluded_argument_warning`, which keeps a `.formatignore` or `.prettierignore` rule's
 * skip quiet). Returns whether a rule excluded it. The safety nets and the build-output
 * heuristic never apply: they prune what a walk discovers, and the caller named this
 * file. The scope is the file's canonical directory's, resolved as a directory root's is,
 * and moved there from the previous file argument's (`enter_file_scope`). Mirrors the
 * native `collect_file`.
 */
function collect_file(path, cwd, scope, files, errors, warnings) {
	const file_abs = absolute_named_path(path, cwd);
	if (file_abs === null) {
		errors.push(unresolvable_root_error(path));
		return false;
	}
	enter_file_scope(scope, dirname(file_abs), warnings);
	const rel = rel_under(scope.format_root, file_abs) ?? '';
	if (!scope.stack.is_ignored(rel, false)) {
		files.push(path);
		return false;
	}
	const warning = scope.stack.excluded_argument_warning(
		path,
		rel,
		false,
		loose_root(scope.format_root, scope.in_repo)
	);
	if (warning !== undefined) warnings.push(warning);
	return true;
}

/** The format root's display path outside a git repo — where a warning names a path
 * absolutely — and `undefined` inside one. Mirrors the native `loose_root`. */
function loose_root(format_root, in_repo) {
	return in_repo ? undefined : format_root;
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
	const root_abs = absolute_named_path(root, cwd);
	if (root_abs === null) {
		errors.push(unresolvable_root_error(root));
		return;
	}
	const { format_root, in_repo } = format_root_of(root_abs);

	const stack = new IgnoreStack();
	// `root` relative to the format root (always an ancestor-or-self of it, so
	// never null); '' means `root` *is* the format root
	const base_rel = rel_under(format_root, root_abs) ?? '';

	// preload the ancestors *above* `root` (format root → `root`'s parent). `root`
	// and everything below reads its own ignore files in collect_recursive from
	// the listing it already fetches, so an ignore-file-free subtree costs no
	// speculative opens; `root` is excluded here to avoid reading its ignores
	// twice. Ancestors above `root` aren't listed, so they keep the direct open.
	const heuristic_active = preload_ancestors(
		stack,
		ancestor_chain(format_root, root_abs).slice(0, -1),
		format_root,
		in_repo,
		warnings
	);

	// A named root is bounded by the ignore files alone, gated once with the full
	// ancestor-walking matcher: a root a rule excludes — through an ancestor (`tsv format
	// build/sub` with a gitignored `build/`) or at itself — puts nothing under it in
	// scope, and the run says so. The recursion's leaf-only query (is_ignored_leaf, inside
	// classify_dir/should_format_file) is exact only once an entry's ancestors are
	// cleared, which this gate also secures for `root`. The safety nets and the
	// build-output heuristic grade neither the root nor its ancestors, in either regime:
	// they prune what a walk discovers, and the caller named this directory. Mirrors the
	// native collect_root.
	if (stack.is_ignored(base_rel, true)) {
		const warning = stack.excluded_argument_warning(
			root,
			base_rel,
			true,
			loose_root(format_root, in_repo)
		);
		if (warning !== undefined) warnings.push(warning);
		free_ignore_stack(stack);
		return;
	}

	collect_recursive(
		root,
		root_abs,
		base_rel,
		true,
		loose_root(format_root, in_repo),
		stack,
		heuristic_active,
		files,
		errors,
		warnings
	);
	// freed deterministically (both exits) rather than left to GC: a wasm-backed
	// handle finalized after a trap-triggered `reinstantiate` would hold a
	// pointer into the discarded instance, and the registry's guard then leaks
	// it — freeing here keeps discovery handle-clean before formatting can trap.
	// (A throw out of discovery aborts the run before any format call, so no
	// leaked registration can ever meet a reinstantiated engine.)
	free_ignore_stack(stack);
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

/** A path's components spelled as Rust's `Path::components()` yields them: a
 * leading separator is the root component (its own spelling, `/`), a `.`
 * survives only at the head, and every other `.` and empty component (a
 * doubled or trailing separator) is normalized away — so `t//b/y.ts`,
 * `t/./b/y.ts` and `t/b/y.ts` are one sequence. What `compare_paths` orders;
 * the emitted paths keep the argument's spelling, this only decides their order.
 * A Windows UNC path's two leading separators fold to one root component where
 * Rust reads a single `Prefix`, which, like the drive-prefix caveat on
 * `compare_paths`, reaches the same verdict for every pair sharing that root. */
function path_components(path) {
	const raw = split_path_components(path);
	const components = [];
	for (let i = 0; i < raw.length; i++) {
		const c = raw[i];
		if (c === '') {
			if (i === 0 && raw.length > 1) components.push(sep);
		} else if (c !== '.' || i === 0) {
			components.push(c);
		}
	}
	return components;
}

/** Component-wise path ordering matching the native CLI's `path_sort_key` — a
 * separator splits components (normalized by `path_components`, so a doubled
 * or `.` step in the argument's spelling moves nothing), each compares by its
 * own spelling (the root as `/`, `.` and `..` as written), and a shorter
 * prefix sorts first, so `a/y.ts` precedes `a-b/x.ts` (plain string order
 * would invert them: `-` < `/`). The parity claim is scoped to ASCII/BMP
 * names: JS compares UTF-16 code units while Rust compares UTF-8 bytes, so
 * astral-plane names (≥ U+10000) order differently. A Windows path's drive
 * prefix rides along as one leading component where Rust splits it into
 * `Prefix` + `RootDir`, which reaches the same verdict for every pair sharing
 * a root. */
function compare_paths(a, b) {
	if (a === b) return 0;
	const as = path_components(a);
	const bs = path_components(b);
	const len = Math.min(as.length, bs.length);
	for (let i = 0; i < len; i++) {
		if (as[i] !== bs[i]) return as[i] < bs[i] ? -1 : 1;
	}
	return as.length - bs.length;
}

function collect_recursive(
	dir,
	// `dir`'s absolute path — the spelling its ignore files are read by and every
	// ignore-file diagnostic names it by (see tsv_layer_content); `dir` keeps the
	// argument's spelling for the paths the walk emits
	dir_abs,
	dir_rel,
	is_target_root,
	// the format root's display path outside a git repo, `undefined` inside one — what a
	// warning names a path by (loose_root), and so also whether the format root is a git
	// repo (`.gitignore` is read only then). Mirrors the native collect_recursive
	loose_root,
	stack,
	heuristic_active,
	files,
	errors,
	warnings
) {
	const in_repo = loose_root === undefined;
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
	const presence = { formatignore: false, prettierignore: false, gitignore: 'absent' };
	for (const e of entries) {
		if (e.name === GITIGNORE_FILE) {
			// `.gitignore` takes git's presence rule (gitignore_presence), which the
			// listing's own entry type answers: a link is not followed
			if (in_repo) {
				presence.gitignore = e.isSymbolicLink() ? 'symlink' : e.isFile() ? 'file' : 'absent';
			}
			continue;
		}
		if (e.name !== FORMATIGNORE_FILE && e.name !== PRETTIERIGNORE_FILE) continue;
		// the preload's presence rule (is_ignore_file): a listing's file type does not
		// follow a symlink, so only a link costs the stat that asks what it points at
		if (!e.isFile() && !(e.isSymbolicLink() && is_ignore_file(join(dir_abs, e.name)))) continue;
		if (e.name === FORMATIGNORE_FILE) presence.formatignore = true;
		else presence.prettierignore = true;
	}
	// the ladder itself is push_layers, shared with the ancestor preload
	// (push_dir_layers) — this path differs only in having read presence off the listing
	// instead of probing for it
	const pushed = push_layers(stack, dir_abs, dir_rel, in_repo, presence, warnings);
	// outside a git repo a target-root `.prettierignore` is silently skipped (tsv
	// reads `.formatignore` there) — warn (decision + text from Rust, single source
	// of truth with the native CLI), pointing at the rename / `git init` fixes. The
	// preload raises it for no ancestor, which is why probe_ignore_presence can leave
	// `prettierignore` unasked outside a repo where this reads it.
	if (is_target_root) {
		const warning = stack.prettierignore_outside_repo_warning(
			dir_abs,
			in_repo,
			presence.prettierignore,
			presence.formatignore
		);
		if (warning != null) warnings.push(warning);
	}
	// this dir's own `.gitignore`, if push_layers pushed one, turns the heuristic off for
	// its children — an unreadable or symlinked one pushes nothing, so the heuristic stays
	// on for the subtree and the warning it raised is what makes that visible
	const child_heuristic = heuristic_active && !pushed.gitignore;

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
				if (verdict === 'prune_warn') {
					const warning = stack.heuristic_shadow_warning(child_rel, loose_root);
					if (warning !== undefined) warnings.push(warning);
				}
				continue;
			}
			// the child reads its own ignore files when we recurse into it
			collect_recursive(
				path,
				join(dir_abs, entry.name),
				child_rel,
				false,
				loose_root,
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

	pop_layers(stack, pushed);
}

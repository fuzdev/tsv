#!/usr/bin/env node
/**
 * `tsv` bin for `@fuzdev/tsv_wasm` — mirrors the native `tsv_cli` contract
 * (subcommands, flags, exit codes, output streams, traversal rules) over the
 * WASM build. Single-threaded — `--jobs` is accepted for drop-in parity and
 * ignored; the native CLI is the fast path for large trees.
 *
 * Exit codes: `format` — 0 clean, 1 would-change (`--check`), 2 errors;
 * `parse` — 0 ok, 1 error. Argument-parsing errors exit 1 (both commands).
 */

import {
	existsSync,
	readdirSync,
	readFileSync,
	realpathSync,
	statSync,
	writeFileSync,
} from 'node:fs';
import { dirname, isAbsolute, join, relative as path_relative, sep } from 'node:path';
import { parseArgs } from 'node:util';
import {
	format_css,
	format_svelte,
	format_typescript,
	IgnoreStack,
	parse_css_json,
	parse_svelte_json,
	parse_typescript_json,
} from './index.js';

/** tsv's native ignore file, discovered hierarchically (one per directory).
 * Mirrors `FORMATIGNORE_FILE`. */
const FORMATIGNORE_FILE = '.formatignore';

/** Prettier's ignore file — read only at the repo root for drop-in compat, and
 * shadowed by *presence* of a repo-root `.formatignore` (used alone when present,
 * even if present-but-unreadable). Never hierarchical, and never outside a repo —
 * there a target-root `.prettierignore` triggers a heads-up warning instead.
 * Mirrors `PRETTIERIGNORE_FILE`. */
const PRETTIERIGNORE_FILE = '.prettierignore';

/** The git ignore file, discovered hierarchically (one per directory) and only
 * inside a git repo, matching git. */
const GITIGNORE_FILE = '.gitignore';

const FORMATTERS = {
	svelte: format_svelte,
	typescript: format_typescript,
	css: format_css,
};

const PARSERS = {
	svelte: parse_svelte_json,
	typescript: parse_typescript_json,
	css: parse_css_json,
};

/** Valid `--parser` values (shared by `format` and `parse` — `FORMATTERS` and
 * `PARSERS` are keyed by the same names). */
const PARSER_NAMES = new Set(['svelte', 'typescript', 'css']);

const HELP = `Usage: tsv <command> [<args>]

formatter and parser for Svelte, TypeScript, and CSS (WASM build)

Commands:
  format            Format source code in place (near-Prettier output)
  parse             Parse source code into AST JSON
  help              Print help for a command

Run \`tsv <command> --help\` for command flags.
`;

const FORMAT_HELP =
	`Usage: tsv format [<paths...>] [--check] [--list] [--content <s> | --stdin] [--parser <p>]

Format source code in place (near-Prettier output).

Paths are formatted in place (written only when the output differs) and
changed paths print to stdout; directories recurse over .ts/.svelte/.css,
honoring .gitignore (hierarchically, in a git tree) plus a repo-root
.formatignore / .prettierignore. An explicitly named file is always formatted.
--content/--stdin print formatted source to stdout.

Options:
  --content <s>     content to format, printed to stdout (requires --parser)
  --stdin           read from stdin, print to stdout (requires --parser)
  --parser <p>      parser type: svelte | typescript | css (--content/--stdin only)
  --check           check instead of writing/printing: exit 1 if any input would change
  --list            list the discovered in-scope files (one per line) without formatting; path mode only
  --jobs <n>        accepted for native-CLI parity and ignored (single-threaded)

Exit codes: 0 clean, 1 would change (--check), 2 errors.
`;

const PARSE_HELP = `Usage: tsv parse [<file>] [--pretty] [--content <s> | --stdin] [--parser <p>]

Parse source code into AST JSON.

Options:
  --pretty          pretty-print JSON output
  --content <s>     content to parse (requires --parser)
  --stdin           read from stdin (requires --parser)
  --parser <p>      parser type: svelte | typescript | css
`;

main();

function main() {
	const [command, ...rest] = process.argv.slice(2);
	switch (command) {
		case 'format':
			run_format(rest);
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
		case undefined:
			eprint(HELP);
			process.exit(1);
			break;
		default:
			eprint(`Error: unknown command '${command}'\n\n${HELP}`);
			process.exit(1);
	}
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

/** Extension-based parser detection, mirroring the native `ParserType::from_extension`. */
function parser_from_extension(path) {
	if (path.endsWith('.svelte')) return 'svelte';
	if (path.endsWith('.css')) return 'css';
	return 'typescript';
}

function run_format(args) {
	const { values, positionals } = parse_argv(
		args,
		{
			content: { type: 'string' },
			stdin: { type: 'boolean' },
			parser: { type: 'string' },
			check: { type: 'boolean' },
			list: { type: 'boolean' },
			jobs: { type: 'string' },
			help: { type: 'boolean' },
		},
		FORMAT_HELP,
	);

	// validated before mode dispatch — a bad value exits 1 in every mode (argh parity)
	const parser = values.parser === undefined ? undefined : resolve_parser(values.parser);
	// --jobs is accepted for drop-in parity with the native CLI and otherwise
	// ignored (single-threaded): a non-integer value is an argument-parsing
	// error (exit 1, argh parity), and combining it with --content/--stdin is
	// rejected in format_single like the native CLI (exit 2)
	if (values.jobs !== undefined && !/^\d+$/.test(values.jobs)) {
		eprint(`Error: --jobs expects an integer, got '${values.jobs}'\n`);
		process.exit(1);
	}
	if (values.content !== undefined || values.stdin) {
		format_single(values, positionals, parser);
	} else {
		format_paths(values, positionals);
	}
}

/** `--content`/`--stdin` mode — format one input to stdout (or `--check` it). */
function format_single(values, positionals, parser) {
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
		formatted = FORMATTERS[parser](input);
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

/** Path mode — discover files, format sequentially, report in sorted order. */
function format_paths(values, positionals) {
	if (positionals.length === 0) {
		eprint('Error: No input provided. Use a file path, --content, or --stdin\n');
		process.exit(2);
	}
	if (values.parser !== undefined) {
		eprint(
			'Error: --parser applies to --content/--stdin; file paths use extension detection\n',
		);
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
		// neutral wording: an empty result can mean "no .ts/.svelte/.css here" *or*
		// "all of them are ignored" (e.g. a target under a gitignored dir), so don't
		// imply a wrong-extension cause. `--list` reports the empty set and exits 0.
		eprint('Error: No files to format — no unignored .ts/.svelte/.css files in scope\n');
		process.exit(2);
	}

	let changed = 0;
	let unchanged = 0;
	let errors = traversal_errors.length;
	for (const path of files) {
		let source;
		try {
			source = readFileSync(path, 'utf-8');
		} catch (error) {
			errors++;
			eprint(`error: ${path}: read failed: ${error.message}\n`);
			continue;
		}
		let formatted;
		try {
			formatted = FORMATTERS[parser_from_extension(path)](source);
		} catch (error) {
			errors++;
			eprint(`error: ${path}: ${error.message}\n`);
			continue;
		}
		if (formatted === source) {
			unchanged++;
			continue;
		}
		if (!values.check) {
			try {
				writeFileSync(path, formatted);
			} catch (error) {
				errors++;
				eprint(`error: ${path}: write failed: ${error.message}\n`);
				continue;
			}
		}
		changed++;
		print(`${path}\n`);
	}

	const action = values.check ? 'would change' : 'formatted';
	const error_note = errors > 0 ? `, ${errors} errors` : '';
	eprint(`${changed} ${action}, ${unchanged} unchanged${error_note}\n`);

	if (errors > 0) process.exit(2);
	if (values.check && changed > 0) process.exit(1);
}

function run_parse(args) {
	const { values, positionals } = parse_argv(
		args,
		{
			pretty: { type: 'boolean' },
			content: { type: 'string' },
			stdin: { type: 'boolean' },
			parser: { type: 'string' },
			help: { type: 'boolean' },
		},
		PARSE_HELP,
	);

	// validated before mode dispatch — a bad value exits 1 in every mode (argh parity)
	const flag_parser = values.parser === undefined ? undefined : resolve_parser(values.parser);
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

	let json;
	try {
		json = PARSERS[parser](input);
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
 * upfront (any bad one fails the run with exit 2), explicit files are always
 * included regardless of extension *and* regardless of the ignore files (the
 * caller named them), and directories recurse with the extension filter.
 * Symlinks inside directories are not followed. Traversal errors below a valid
 * root are non-fatal and returned for reporting. See `collect_root` for the
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
	const bad = paths.filter((_, i) => !stats[i]?.isFile() && !stats[i]?.isDirectory());
	if (bad.length > 0) {
		for (const path of bad) {
			eprint(`error: ${path}: not a file or directory\n`);
		}
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
			files.push(paths[i]); // explicit file bypasses the ignore files
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
 * ancestor chain from there down: `.formatignore` at each level (with a repo-root
 * `.prettierignore` shadow), and `.gitignore` at each level when in a repo.
 * Mirrors the native `collect_root`.
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
		// tsv layer: `.formatignore` everywhere; at the repo root only, a
		// `.prettierignore` it shadows. No listing for ancestors, so the read
		// outcome stands in for presence: a present-but-unreadable `.formatignore`
		// (`unreadable`, warned) shadows `.prettierignore` just as in the listed
		// case — only a genuinely `absent` one falls through.
		let tsv = null;
		if (in_repo && anchor === '') {
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

	collect_recursive(root, base_rel, true, in_repo, stack, heuristic_active, files, errors, warnings);
}

/** Component-wise path ordering matching Rust's `PathBuf` ordering — `/`
 * splits components and a shorter prefix sorts first, so `a/y.ts` precedes
 * `a-b/x.ts` (plain string order would invert them: `-` < `/`). The parity
 * claim is scoped to ASCII/BMP names and `/`-separated paths: JS compares
 * UTF-16 code units while Rust compares UTF-8 bytes, so astral-plane names
 * (≥ U+10000) order differently, and a backslash-spelled Windows root
 * doesn't split into components. */
function compare_paths(a, b) {
	if (a === b) return 0;
	const as = a.split('/');
	const bs = b.split('/');
	const len = Math.min(as.length, bs.length);
	for (let i = 0; i < len; i++) {
		if (as[i] !== bs[i]) return as[i] < bs[i] ? -1 : 1;
	}
	return as.length - bs.length;
}

function collect_recursive(dir, dir_rel, is_target_root, in_repo, stack, heuristic_active, files, errors, warnings) {
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
	// repo); at the repo root only, a `.prettierignore` is the drop-in fallback,
	// used solely when no `.formatignore` is present. Precedence is by presence
	// (the listing), not readability — a present-but-unreadable `.formatignore`
	// still shadows: read_ignore_file warns and yields no rules.
	let tsv = null;
	if (has_formatignore) {
		const r = read_ignore_file(join(dir, FORMATIGNORE_FILE), warnings);
		if (r.kind === 'content') tsv = r.content;
	} else if (in_repo && dir_rel === '' && has_prettierignore) {
		const r = read_ignore_file(join(dir, PRETTIERIGNORE_FILE), warnings);
		if (r.kind === 'content') tsv = r.content;
	}
	let tsv_pushed = false;
	if (tsv !== null) {
		stack.push_tsv(dir_rel, tsv);
		tsv_pushed = true;
	}
	// outside a git repo a target-root `.prettierignore` is silently skipped (tsv
	// reads `.formatignore` there) — warn (decision + text from Rust, single source
	// of truth with the native CLI), pointing at the rename / `git init` fixes.
	if (is_target_root) {
		const warning = stack.prettierignore_outside_repo_warning(
			dir,
			in_repo,
			has_prettierignore,
			has_formatignore,
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
		// PathBuf::push parity: only insert a separator when the dir doesn't
		// already end with one, so a trailing-slash root (`tsv format src/`)
		// yields `src/a.ts`, not `src//a.ts`
		const path = dir.endsWith('/') ? `${dir}${entry.name}` : `${dir}/${entry.name}`;
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
			collect_recursive(path, child_rel, false, in_repo, stack, child_heuristic, files, errors, warnings);
		} else if (entry.isFile() && stack.should_format_file(entry.name, child_rel)) {
			files.push(path);
		}
	}

	if (git_pushed) stack.pop_gitignore();
	if (tsv_pushed) stack.pop_tsv();
}

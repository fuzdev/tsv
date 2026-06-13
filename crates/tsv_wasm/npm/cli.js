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

import { readdirSync, readFileSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { parseArgs } from 'node:util';
import {
	format_css,
	format_svelte,
	format_typescript,
	parse_css_json,
	parse_svelte_json,
	parse_typescript_json,
} from './index.js';

/** Directory names skipped during recursive discovery, in addition to hidden
 * directories (leading `.`), which are skipped unconditionally. */
const EXCLUDED_DIRS = new Set(['node_modules', 'dist', 'build', 'target']);

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
	`Usage: tsv format [<paths...>] [--check] [--content <s> | --stdin] [--parser <p>]

Format source code in place (near-Prettier output).

Paths are formatted in place (written only when the output differs) and
changed paths print to stdout; directories recurse over .ts/.svelte/.css.
--content/--stdin print formatted source to stdout.

Options:
  --content <s>     content to format, printed to stdout (requires --parser)
  --stdin           read from stdin, print to stdout (requires --parser)
  --parser <p>      parser type: svelte | typescript | css (--content/--stdin only)
  --check           check instead of writing/printing: exit 1 if any input would change
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

	const { files, errors: traversal_errors } = discover_files(positionals);
	for (const msg of traversal_errors) {
		eprint(`error: ${msg}\n`);
	}
	if (files.length === 0 && traversal_errors.length === 0) {
		eprint('Error: No supported files found (.ts, .svelte, .css)\n');
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

/** Whether a file name has a formattable extension (compound forms like
 * `.svelte.ts` are covered by the `.ts` match). A leading dot is part of the
 * stem, not an extension — a file named exactly `.ts` doesn't match, same as
 * Rust's `Path::extension`. */
function is_formattable(name) {
	return /.\.(ts|svelte|css)$/.test(name);
}

/**
 * Expand files and directories into a sorted, deduplicated list of files to
 * format, mirroring the native `discover_files`: root args are validated
 * upfront (any bad one fails the run with exit 2), explicit files are always
 * included regardless of extension, directories recurse with the extension
 * filter skipping hidden directories and `EXCLUDED_DIRS`, and symlinks inside
 * directories are not followed. Traversal errors below a valid root are
 * non-fatal and returned for reporting.
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

	let files = [];
	const errors = [];
	for (let i = 0; i < paths.length; i++) {
		if (stats[i].isFile()) {
			files.push(paths[i]);
		} else {
			collect_recursive(paths[i], files, errors);
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
	return { files, errors };
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

function collect_recursive(dir, files, errors) {
	let entries;
	try {
		entries = readdirSync(dir, { withFileTypes: true });
	} catch (error) {
		errors.push(`${dir}: read_dir failed: ${error.message}`);
		return;
	}
	for (const entry of entries) {
		// PathBuf::push parity: only insert a separator when the dir doesn't
		// already end with one, so a trailing-slash root (`tsv format src/`)
		// yields `src/a.ts`, not `src//a.ts`
		const path = dir.endsWith('/') ? `${dir}${entry.name}` : `${dir}/${entry.name}`;
		if (entry.isDirectory()) {
			if (entry.name.startsWith('.') || EXCLUDED_DIRS.has(entry.name)) continue;
			collect_recursive(path, files, errors);
		} else if (entry.isFile() && is_formattable(entry.name)) {
			files.push(path);
		}
	}
}

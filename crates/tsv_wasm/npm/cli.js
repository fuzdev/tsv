#!/usr/bin/env node
/**
 * `tsv` bin for `@fuzdev/tsv_wasm` — mirrors the native `tsv_cli` contract
 * (subcommands, flags, exit codes, output streams, traversal rules) over the
 * WASM build. Single-threaded (no `--jobs`); the native CLI is the fast path
 * for large trees.
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

const HELP = `Usage: tsv <command> [<args>]

formatter and parser for TypeScript, Svelte, and CSS (WASM build)

Commands:
  format            Format source code in place (matches Prettier output)
  parse             Parse source code into AST JSON

Run \`tsv <command> --help\` for command flags.
`;

const FORMAT_HELP =
	`Usage: tsv format [<paths...>] [--check] [--content <s> | --stdin] [--parser <p>]

Format source code in place (matches Prettier output).

Paths are formatted in place (written only when the output differs) and
changed paths print to stdout; directories recurse over .ts/.svelte/.css.
--content/--stdin print formatted source to stdout.

Options:
  --content <s>     content to format, printed to stdout (requires --parser)
  --stdin           read from stdin, print to stdout (requires --parser)
  --parser <p>      parser type: svelte | typescript | css (--content/--stdin only)
  --check           check instead of writing/printing: exit 1 if any input would change

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
		case '--help':
		case '-h':
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

/** Resolve a `--parser` value (`ts` is an accepted alias), or exit. */
function resolve_parser(name, exit_code) {
	const resolved = name === 'ts' ? 'typescript' : name;
	if (!(resolved in FORMATTERS)) {
		eprint(
			`Error: Unknown parser type: '${name}'. Valid types: svelte, typescript, css\n`,
		);
		process.exit(exit_code);
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
			help: { type: 'boolean', short: 'h' },
		},
		FORMAT_HELP,
	);

	if (values.content !== undefined || values.stdin) {
		format_single(values, positionals);
	} else {
		format_paths(values, positionals);
	}
}

/** `--content`/`--stdin` mode — format one input to stdout (or `--check` it). */
function format_single(values, positionals) {
	if (positionals.length > 0) {
		eprint('Error: --content/--stdin cannot be combined with file paths\n');
		process.exit(2);
	}
	const flag = values.content !== undefined ? '--content' : '--stdin';
	if (values.parser === undefined) {
		eprint(`Error: ${flag} requires --parser <svelte|typescript|css>\n`);
		process.exit(2);
	}
	const parser = resolve_parser(values.parser, 2);
	const input = values.content !== undefined ? values.content : read_stdin();
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
			help: { type: 'boolean', short: 'h' },
		},
		PARSE_HELP,
	);

	// Input precedence mirrors the native `InputArgs::resolve`: --content > --stdin > file.
	let input;
	let parser;
	if (values.content !== undefined) {
		if (values.parser === undefined) {
			eprint('Error: --content requires --parser <svelte|typescript|css>\n');
			process.exit(1);
		}
		parser = resolve_parser(values.parser, 1);
		input = values.content;
	} else if (values.stdin) {
		if (values.parser === undefined) {
			eprint('Error: --stdin requires --parser <svelte|typescript|css>\n');
			process.exit(1);
		}
		parser = resolve_parser(values.parser, 1);
		input = read_stdin();
	} else if (positionals.length > 0) {
		const path = positionals[0];
		parser = values.parser === undefined
			? parser_from_extension(path)
			: resolve_parser(values.parser, 1);
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
		json = JSON.stringify(JSON.parse(json), null, '\t');
	}
	print(`${json}\n`);
}

/** Synchronous stdout write — `process.stdout.write` is async on pipes, so a
 * `process.exit` right after it can truncate output. */
function print(text) {
	writeFileSync(1, text);
}

/** Synchronous stderr write (same truncation hazard as `print`). */
function eprint(text) {
	writeFileSync(2, text);
}

function read_stdin() {
	try {
		return readFileSync(0, 'utf-8');
	} catch (error) {
		eprint(`Error: Error reading from stdin: ${error.message}\n`);
		process.exit(2);
	}
}

/** Whether a path has a formattable extension (compound forms like
 * `.svelte.ts` are covered by the `.ts` match). */
function is_formattable(path) {
	return /\.(ts|svelte|css)$/.test(path);
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
	files.sort();
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

function collect_recursive(dir, files, errors) {
	let entries;
	try {
		entries = readdirSync(dir, { withFileTypes: true });
	} catch (error) {
		errors.push(`${dir}: read_dir failed: ${error.message}`);
		return;
	}
	for (const entry of entries) {
		const path = `${dir}/${entry.name}`;
		if (entry.isDirectory()) {
			if (entry.name.startsWith('.') || EXCLUDED_DIRS.has(entry.name)) continue;
			collect_recursive(path, files, errors);
		} else if (entry.isFile() && is_formattable(entry.name)) {
			files.push(path);
		}
	}
}

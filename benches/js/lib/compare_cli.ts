/**
 * Shared CLI scaffolding for the comparison tools (`corpus_compare_format.ts`,
 * `corpus_compare_parse.ts`, the fixtures gates, `diagnostics/ts_repo_compare.ts`): common
 * argument fields, path/filter resolution, the loader selection, the engines a run holds and
 * their teardown, the shared panic gate, and the `--json` stdout/stderr discipline.
 *
 * Keeping this in one place means hardening (freshness-guard behavior, sidecar init errors)
 * applies to every comparison tool instead of drifting per-script — and a GATE that lives here
 * cannot be answered two ways by two tools, which is how the panic hole opened in the first
 * place.
 *
 * The tools are library functions over deps returning an exit code; their roots under
 * `benches/js/roots/` bind gro's host modules and load the engines
 * (`compare_engines.ts`). This module loads none of them: the engines are the structural
 * types below, so a unit suite drives a tool over fakes.
 *
 * @module
 */

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import * as s from '@fuzdev/fuz_schema/schema.ts';
import { meta } from '@fuzdev/fuz_schema/meta.ts';

import { CorpusLoader, DirectoryLoader } from './corpus.ts';
import { is_native_panic_error } from './divergence/panic_errors.ts';
import { type Language, LANGUAGES, type ParseGoal } from './types.ts';

/**
 * Schema fields shared by every corpus_compare_* tool — spread into the tool's
 * own args schema alongside its tool-specific flags.
 */
export const COMPARE_BASE_ARG_FIELDS = {
	_: s.with_default_fn(s.array(s.string), () => []),
	all: meta(s.with_default(s.boolean, false), { aliases: ['a'] }),
	filter: meta(s.optional(s.string), { aliases: ['f'] }),
	limit: meta(s.optional(s.number), { aliases: ['l'] }),
	verbose: meta(s.with_default(s.boolean, false), { aliases: ['v'] }),
	json: s.with_default(s.boolean, false),
	/** Rewrite the gate's per-source pins file from this run (`lib/corpus_pins.ts`) — `--all`, unnarrowed. */
	update: s.with_default(s.boolean, false),
	help: meta(s.with_default(s.boolean, false), { aliases: ['h'] })
} as const;

/** Where a tool writes: the process's stdout, which `--json` reserves for its report. */
export interface CompareOutput {
	write(data: string): Promise<unknown>;
}

/** A directory a tool reads, read-only, by paths beneath it (a gro `FsReadDir` is one). */
export interface CompareReadDir {
	/** The entries at `path` (the directory itself by default); rejects when it can't list. */
	list(path?: string): AsyncIterable<{ name: string; kind: string }>;
	read_text(path: string): Promise<string>;
}

/** The canonical oracles a comparison run holds (`lib/canonical.ts`'s implementation). */
export interface CanonicalEngine {
	format_async(source: string, language: Language, source_path?: string): Promise<string>;
	parse(source: string, language: Language, goal?: ParseGoal): unknown;
	/** The prettier-output cache, or `null` when disabled; its `stats()` line is printed. */
	enable_format_cache(): { stats(): string } | null;
}

/** tsv, through the N-API addon a run loads (`lib/napi.ts`). */
export interface NativeEngine {
	format(source: string, language: Language): string;
	parse(source: string, language: Language, goal?: ParseGoal): unknown;
	parse_internal(source: string, language: Language, goal?: ParseGoal): void;
}

/** The pair of engines a comparison run holds open for its lifetime, and their release. */
export interface CompareImplementations {
	canonical: CanonicalEngine;
	native: NativeEngine;
	dispose(): void;
}

/**
 * Loads a run's engines behind the artifact-freshness guard: `null` when one failed to load,
 * having said why and how to rebuild it, so the run fails with exit 1.
 */
export type InitCompareImplementations = () => Promise<CompareImplementations | null>;

/** Get relative path from base directory. */
export function rel_path(file_path: string, base: string): string {
	return file_path.startsWith(base + '/') ? file_path.slice(base.length + 1) : file_path;
}

/**
 * Resolve the comparison base path: `~` expansion, or `~/dev` under --all.
 * @param home - `HOME`, read at the tool's root
 */
export function resolve_compare_base_path(
	path: string | undefined,
	use_all: boolean,
	home: string | undefined
): string {
	const home_dir = home ?? '';
	return use_all ? `${home_dir}/dev` : path!.startsWith('~') ? path!.replace('~', home_dir) : path!;
}

/**
 * Validate a --filter value as a `Language`: `{ok: false}` once the error is printed, so the
 * run fails with exit 1.
 */
export function parse_language_filter(
	filter: string | undefined
): { ok: true; language: Language | undefined } | { ok: false } {
	if (filter === undefined) return { ok: true, language: undefined };
	const lang = filter as Language;
	if (!LANGUAGES.includes(lang)) {
		console.error(`Error: Invalid filter "${filter}". Must be one of: ${LANGUAGES.join(', ')}`);
		return { ok: false };
	}
	return { ok: true, language: lang };
}

/**
 * In --json mode, stdout is reserved for the buffered JSON report — reroute
 * all console.log human/progress output to stderr.
 */
export function redirect_logs_to_stderr(): void {
	console.log = (...a: unknown[]) => console.error(...a);
}

/** Write the buffered JSON report to stdout (the only thing on stdout in --json mode). */
export async function emit_json_stdout(
	stdout: CompareOutput,
	report: Record<string, unknown>
): Promise<void> {
	await stdout.write(JSON.stringify(report, null, '\t') + '\n');
}

/**
 * Create the corpus loader: the `gates` view under --all (the pre-split
 * corpus the sanction lists and divergence coverage were reviewed against —
 * see `corpus.ts`), else one directory.
 */
export function create_compare_loader(
	use_all: boolean,
	base_path: string
): CorpusLoader | DirectoryLoader {
	return use_all ? new CorpusLoader('gates') : new DirectoryLoader(base_path);
}

/**
 * Release the engines and fail — the shared failure contract of every gate in a comparison
 * tool. Exists so a gate added later can't half-remember it: the only way to fail a run is
 * the way that also releases the oracles and the addon's bindings.
 * @returns the run's exit code, `1`
 */
export function fail_compare(impls: CompareImplementations): 1 {
	impls.dispose();
	return 1;
}

/** A per-file failure a panic gate can grade: where it happened, and what threw. */
export interface CompareFailure {
	path: string;
	error?: string;
}

/**
 * HARD-FAIL the run on any caught panic, on every run — not just `--all`, and
 * never folded into a per-file bucket. Returns whether it tripped, the report printed; the
 * caller then fails the run (`fail_compare`).
 *
 * The comparison tools load tsv built with `--profile corpus` (`panic = "unwind"`)
 * precisely so a crash is caught and reported per file rather than killing the
 * run; the SHIPPED artifacts are `panic = "abort"` and take the host process
 * down on that same input. So a caught panic arrives in the run's mildest
 * bucket while describing the release's harshest failure (robustness bar tier
 * 2) — `errors` → a WARN at exit 0 in the format tool, a dimmed `parse-fail
 * skipped` line in the parse tool.
 *
 * Callers pass the failures that could be tsv's (the parse tool's canonical-side
 * errors are excluded there, being JS); the CLASSIFICATION lives here, in one
 * place, so the two tools cannot answer this question differently.
 *
 * Call it AFTER the report body is printed — a panic is the headline, not a
 * reason to withhold the run's findings.
 */
export function gate_on_panics(failures: readonly CompareFailure[], base_path: string): boolean {
	const panics = failures.filter((f) => is_native_panic_error(f.error));
	if (panics.length === 0) return false;

	console.log(
		`\n\x1b[31mFAIL: ${panics.length} panic(s) — tsv crashed on real code ` +
			`(caught here by the corpus profile; a shipped artifact would abort the host):\x1b[0m`
	);
	for (const f of panics.slice(0, 10)) {
		console.log(`  ${rel_path(f.path, base_path)}: ${f.error}`);
	}
	if (panics.length > 10) console.log(`  ... and ${panics.length - 10} more`);
	return true;
}

/**
 * Run a comparison tool's entry, enforcing the shared failure contract on a throw: the
 * message goes to stderr, and in --json mode stdout still receives a parseable error-shaped
 * report. The --json flag is re-derived from `argv` here because a rejection can happen
 * before (or during) the entry's own arg parse.
 * @returns the entry's exit code, or `1` when it threw
 */
export async function run_compare_main<T extends { json: boolean }>(
	main: () => Promise<number>,
	options: {
		argv: readonly string[];
		schema: s.Schema<T, any>;
		build_error_report: (message: string) => Record<string, unknown>;
		stdout: CompareOutput;
	}
): Promise<number> {
	try {
		return await main();
	} catch (e) {
		const message = e instanceof Error ? e.message : String(e);
		console.error(message);
		const parsed = args_parse(argv_parse([...options.argv]), options.schema);
		if (parsed.ok && parsed.value.json) {
			await emit_json_stdout(options.stdout, options.build_error_report(message));
		}
		return 1;
	}
}

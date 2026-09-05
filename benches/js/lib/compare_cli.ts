/**
 * Shared CLI scaffolding for the corpus_compare_* entry points
 * (corpus_compare_format.ts, corpus_compare_parse.ts): common argument
 * fields, path/filter resolution, loader selection, implementation init with
 * the artifact-freshness guard, the teardown/failure contract, the shared panic
 * gate, and the --json stdout/stderr discipline.
 *
 * Keeping this in one place means hardening (the FFI heisenbug mitigations,
 * freshness-guard behavior, sidecar init errors) applies to every comparison
 * tool instead of drifting per-script — and a GATE that lives here cannot be
 * answered two ways by two tools, which is how the panic hole opened in the
 * first place.
 */

import process from 'node:process';

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import { z } from 'zod';

import { CorpusLoader, DirectoryLoader } from './corpus.ts';
import { CanonicalImplementation } from './canonical.ts';
import { is_native_panic_error } from './divergence/panic_errors.ts';
import { NativeImplementation } from './ffi.ts';
import { check_artifact_freshness, native_artifact_check } from './check_artifact_freshness.ts';
import { check_node_modules } from './check_node_modules.ts';
import { type Language, LANGUAGES } from './types.ts';
import { load_all_versions } from './versions.ts';

/**
 * Zod fields shared by every corpus_compare_* tool — spread into the tool's
 * own args schema alongside its tool-specific flags.
 */
export const COMPARE_BASE_ARG_FIELDS = {
	_: z.array(z.string()).default(() => []),
	all: z
		.boolean()
		.default(false)
		.meta({ aliases: ['a'] }),
	filter: z
		.string()
		.optional()
		.meta({ aliases: ['f'] }),
	limit: z
		.number()
		.optional()
		.meta({ aliases: ['l'] }),
	verbose: z
		.boolean()
		.default(false)
		.meta({ aliases: ['v'] }),
	json: z.boolean().default(false),
	help: z
		.boolean()
		.default(false)
		.meta({ aliases: ['h'] })
} as const;

/** Get relative path from base directory. */
export function rel_path(file_path: string, base: string): string {
	return file_path.startsWith(base + '/') ? file_path.slice(base.length + 1) : file_path;
}

/** Resolve the comparison base path: `~` expansion, or `~/dev` under --all. */
export function resolve_compare_base_path(path: string | undefined, use_all: boolean): string {
	const home_dir = Deno.env.get('HOME') ?? '';
	return use_all ? `${home_dir}/dev` : path!.startsWith('~') ? path!.replace('~', home_dir) : path!;
}

/** Validate a --filter value as a {@link Language}, exiting with a message if invalid. */
export function parse_language_filter(filter: string | undefined): Language | undefined {
	if (filter === undefined) return undefined;
	const lang = filter as Language;
	if (!LANGUAGES.includes(lang)) {
		console.error(`Error: Invalid filter "${filter}". Must be one of: ${LANGUAGES.join(', ')}`);
		Deno.exit(1);
	}
	return lang;
}

/**
 * In --json mode, stdout is reserved for the buffered JSON report — reroute
 * all console.log human/progress output to stderr.
 */
export function redirect_logs_to_stderr(): void {
	console.log = (...a: unknown[]) => console.error(...a);
}

/** Write the buffered JSON report to stdout (the only thing on stdout in --json mode). */
export function emit_json_stdout(report: Record<string, unknown>): void {
	Deno.stdout.writeSync(new TextEncoder().encode(JSON.stringify(report, null, '\t') + '\n'));
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

/** The pair of implementations a comparison run holds open for its lifetime. */
export interface CompareImplementations {
	canonical: CanonicalImplementation;
	native: NativeImplementation;
}

/**
 * Initialize the canonical (prettier + svelte/compiler + acorn) and native
 * (FFI) implementations behind the artifact-freshness guard, exiting with a
 * profile-aware rebuild hint on failure. Callers own the teardown —
 * `dispose_compare` on the clean path, `exit_compare_failure` on every gate.
 */
export async function init_compare_implementations(): Promise<CompareImplementations> {
	// The canonical impls resolve from benches/js/node_modules — check it exists
	// (and isn't stale) up front so the failure is the installer hint, not an
	// opaque module-resolution error mid-init.
	await check_node_modules();

	// Refuse to measure a stale binary (the `:run` task variants skip the
	// rebuild). Only the native library: these tools run no WASM, so the bundle's
	// freshness is not their business. Override with BENCH_STALE_OK=1. Held in a
	// const because the init failure below quotes the same rebuild hint — one
	// answer to "how do I rebuild this artifact", whether it was stale or unloadable.
	const native_check = native_artifact_check();
	await check_artifact_freshness([native_check]);

	const versions = await load_all_versions();
	const canonical = new CanonicalImplementation(versions.canonical);
	const native = new NativeImplementation();

	try {
		await canonical.init();
	} catch (e) {
		console.error(`Failed to initialize canonical implementations: ${e}`);
		Deno.exit(1);
	}

	try {
		await native.init();
	} catch (e) {
		console.error(`Failed to initialize native implementation: ${e}`);
		console.error(`Run: ${native_check.rebuild}`);
		Deno.exit(1);
	}

	return { canonical, native };
}

/** Release both implementations at the end of a run. */
export function dispose_compare(impls: CompareImplementations): void {
	impls.canonical.dispose();
	impls.native.dispose();
}

/**
 * Tear down and exit 1 — the shared failure contract of every gate in a
 * comparison tool. Exists so a gate added later can't half-remember it: the
 * only way to fail a run is the way that also releases the sidecar and the FFI
 * handle.
 */
export function exit_compare_failure(impls: CompareImplementations): never {
	dispose_compare(impls);
	Deno.exit(1);
}

/** A per-file failure a panic gate can grade: where it happened, and what threw. */
export interface CompareFailure {
	path: string;
	error?: string;
}

/**
 * HARD-FAIL the run on any caught panic, on every run — not just `--all`, and
 * never folded into a per-file bucket.
 *
 * The comparison tools build tsv with `--profile corpus` (`panic = "unwind"`)
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
export function gate_on_panics(
	failures: readonly CompareFailure[],
	impls: CompareImplementations,
	base_path: string
): void {
	const panics = failures.filter((f) => is_native_panic_error(f.error));
	if (panics.length === 0) return;

	console.log(
		`\n\x1b[31mFAIL: ${panics.length} panic(s) — tsv crashed on real code ` +
			`(caught here by the corpus profile; a shipped artifact would abort the host):\x1b[0m`
	);
	for (const f of panics.slice(0, 10)) {
		console.log(`  ${rel_path(f.path, base_path)}: ${f.error}`);
	}
	if (panics.length > 10) console.log(`  ... and ${panics.length - 10} more`);
	exit_compare_failure(impls);
}

/**
 * Run a comparison tool's `main`, enforcing the shared failure contract: the
 * message goes to stderr, and in --json mode stdout still receives a
 * parseable error-shaped report. The --json flag is re-derived from argv here
 * because a rejection can happen before (or during) main()'s own arg parse.
 */
export function run_compare_main<T extends { json: boolean }>(
	main: () => Promise<void>,
	schema: z.ZodType<T>,
	build_error_report: (message: string) => Record<string, unknown>
): void {
	main().catch((e) => {
		const message = e instanceof Error ? e.message : String(e);
		console.error(message);
		const parsed = args_parse(argv_parse(process.argv.slice(2)), schema);
		if (parsed.success && parsed.data.json) {
			emit_json_stdout(build_error_report(message));
		}
		Deno.exit(1);
	});
}

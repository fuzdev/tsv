/**
 * Shared CLI scaffolding for the corpus_compare_* entry points
 * (corpus_compare_format.ts, corpus_compare_parse.ts): common argument
 * fields, path/filter resolution, loader selection, implementation init with
 * the artifact-freshness guard, and the --json stdout/stderr discipline.
 *
 * Keeping this in one place means hardening (the FFI heisenbug mitigations,
 * freshness-guard behavior, sidecar init errors) applies to every comparison
 * tool instead of drifting per-script.
 */

import process from 'node:process';

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import { z } from 'zod';

import { DevReposLoader, DirectoryLoader } from './corpus.ts';
import { CanonicalImplementation } from './canonical.ts';
import { get_library_path, NativeImplementation } from './ffi.ts';
import { check_artifact_freshness } from './check_artifact_freshness.ts';
import { type Language, LANGUAGES } from './types.ts';
import { load_all_versions } from './versions.ts';

/**
 * Zod fields shared by every corpus_compare_* tool — spread into the tool's
 * own args schema alongside its tool-specific flags.
 */
export const COMPARE_BASE_ARG_FIELDS = {
	_: z.array(z.string()).default(() => []),
	all: z.boolean().default(false).meta({ aliases: ['a'] }),
	filter: z.string().optional().meta({ aliases: ['f'] }),
	limit: z.number().optional().meta({ aliases: ['l'] }),
	verbose: z.boolean().default(false).meta({ aliases: ['v'] }),
	json: z.boolean().default(false),
	help: z.boolean().default(false).meta({ aliases: ['h'] }),
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

/** Create the corpus loader: every default repo under --all, else one directory. */
export function create_compare_loader(
	use_all: boolean,
	base_path: string,
): DevReposLoader | DirectoryLoader {
	return use_all ? new DevReposLoader() : new DirectoryLoader(base_path);
}

/**
 * Initialize the canonical (prettier + svelte/compiler + acorn) and native
 * (FFI) implementations behind the artifact-freshness guard, exiting with a
 * profile-aware rebuild hint on failure. Callers own `dispose()` on both.
 */
export async function init_compare_implementations(): Promise<{
	canonical: CanonicalImplementation;
	native: NativeImplementation;
}> {
	// Refuse to measure a stale binary (the `:run` task variants skip the
	// rebuild). See lib/check_artifact_freshness.ts; override with BENCH_STALE_OK=1.
	const ffi_profile = Deno.env.get('TSV_FFI_PROFILE') ?? 'release';
	const rebuild = ffi_profile === 'corpus' ? 'deno task build:ffi:corpus' : 'deno task build:ffi';
	await check_artifact_freshness([
		{
			label: `FFI (${ffi_profile})`,
			path: get_library_path(),
			binding_crates: ['tsv_ffi'],
			rebuild,
		},
	]);

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
		console.error(`Run: ${rebuild}`);
		Deno.exit(1);
	}

	return { canonical, native };
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
	build_error_report: (message: string) => Record<string, unknown>,
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

/**
 * Corpus comparison tool - compares our formatting against Prettier's on arbitrary codebases.
 *
 * Usage:
 *   deno task corpus:compare:format ~/dev/some-project
 *   deno task corpus:compare:format ~/dev/some-project --filter svelte
 *   deno task corpus:compare:format ~/dev/some-project --limit 100
 *   deno task corpus:compare:format ~/dev/some-project --verbose
 *   deno task corpus:compare:format ~/dev/some-project --safety-only
 *   deno task corpus:compare:format ~/dev/some-project --explain
 *   deno task corpus:compare:format ~/dev/some-project --summary   # Compact output (no diffs)
 *   deno task corpus:compare:format --all                          # All default corpus repos
 */

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import { z } from 'zod';

import {
	COMPARE_BASE_ARG_FIELDS,
	create_compare_loader,
	emit_json_stdout,
	init_compare_implementations,
	parse_language_filter,
	redirect_logs_to_stderr,
	rel_path,
	resolve_compare_base_path,
	run_compare_main,
} from './lib/compare_cli.ts';
import {
	diff_lines,
	type DiffHunk,
	extract_hunks,
	filter_diff_context,
	format_diff_for_terminal,
} from './lib/diff.ts';
import {
	CORPUS_FORMAT_MATCH_MIN,
	CORPUS_FORMAT_PARTIAL_PIN,
	CORPUS_FORMAT_UNKNOWN_PIN,
} from './lib/gate_counts.ts';
import { type Language, LANGUAGES } from './lib/types.ts';
import {
	check_expected_error,
	check_safety_vs_prettier,
	detect_divergences,
	type HunkCoverageResult,
	type SafetyViolation,
} from './lib/divergence/mod.ts';

const CorpusCompareArgs = z.object({
	...COMPARE_BASE_ARG_FIELDS,
	'exit-on-first': z.boolean().default(false),
	'safety-only': z.boolean().default(false),
	explain: z.boolean().default(false),
	strict: z.boolean().default(false),
	'audit-patterns': z.boolean().default(false),
	summary: z.boolean().default(false),
});

interface LanguageStats {
	total: number;
	match: number;
	known_divergence: number;
	partial_divergence: number;
	unknown_diff: number;
	safety_violation: number;
	expected_errors: number;
	errors: number;
}

/** Lightweight result — stores path/bytes instead of full SourceFile for GC */
interface CompareResult {
	path: string;
	bytes: number;
	status:
		| 'known_divergence'
		| 'partial_divergence'
		| 'unknown_diff'
		| 'safety_violation'
		| 'expected_error'
		| 'error';
	error?: string;
	/** Reason the error is expected (only for expected_error status) */
	expected_reason?: string;
	/** Only stored for unknown_diff (needed for full diff recomputation) */
	ours?: string;
	/** Only stored for unknown_diff (needed for full diff recomputation) */
	prettier?: string;
	coverage?: HunkCoverageResult;
	safety_violations?: SafetyViolation[];
}

/** Format bytes as human-readable string */
function format_bytes(bytes: number): string {
	if (bytes < 1024) return `${bytes}B`;
	const kb = bytes / 1024;
	if (kb < 10) return `${kb.toFixed(1)}KB`;
	return `${Math.round(kb)}KB`;
}

/** Get a brief diff summary for unknown differences (for agent comprehension) */
function get_diff_summary(prettier: string, ours: string): string {
	const diff = diff_lines(prettier, ours);
	const removals = diff.filter((d) => d.type === 'remove');
	const additions = diff.filter((d) => d.type === 'add');

	// Check for blank line differences
	const blank_removals = removals.filter((d) => !d.line.trim()).length;
	const blank_additions = additions.filter((d) => !d.line.trim()).length;

	// Check for line count difference (we break more/less)
	const prettier_line_count = prettier.split('\n').length;
	const ours_line_count = ours.split('\n').length;
	const line_diff = ours_line_count - prettier_line_count;

	// Find the first meaningful (non-empty) change
	const first_removal = removals.find((d) => d.line.trim())?.line.trim();
	const first_addition = additions.find((d) => d.line.trim())?.line.trim();

	// Describe the difference
	if (line_diff !== 0 && first_removal && first_addition) {
		// Line count changed - likely a breaking difference
		const direction = line_diff > 0 ? 'we break' : 'prettier breaks';
		const snippet = first_removal.slice(0, 40);
		return `${direction} (+${Math.abs(line_diff)} lines): "${snippet}..."`;
	} else if (first_removal && first_addition) {
		// Same line count, content differs
		const r = first_removal.slice(0, 35);
		const a = first_addition.slice(0, 35);
		return `"${r}..." → "${a}..."`;
	} else if (blank_removals !== blank_additions) {
		// Only blank line differences
		if (blank_additions > blank_removals) {
			return `prettier adds ${blank_additions - blank_removals} blank line(s)`;
		} else {
			return `ours adds ${blank_removals - blank_additions} blank line(s)`;
		}
	} else if (first_removal) {
		return `prettier has: "${first_removal.slice(0, 50)}"`;
	} else if (first_addition) {
		return `ours has: "${first_addition.slice(0, 50)}"`;
	}
	return `${removals.length} line(s) differ`;
}

/** A zeroed per-language stats accumulator. */
function empty_stats(): LanguageStats {
	return {
		total: 0,
		match: 0,
		known_divergence: 0,
		partial_divergence: 0,
		unknown_diff: 0,
		safety_violation: 0,
		expected_errors: 0,
		errors: 0,
	};
}

/** All stored results across languages with the given status (sorted callers chain `.sort`). */
function results_by_status(
	results: Map<Language, CompareResult[]>,
	status: CompareResult['status'],
): CompareResult[] {
	return LANGUAGES.flatMap((lang) => results.get(lang)!.filter((r) => r.status === status));
}

/** Run hunk-aware divergence detection for one file: diff → hunks → coverage. */
function run_detection(content: string, ours: string, prettier: string, language: Language) {
	const diff = diff_lines(prettier, ours);
	const hunks = extract_hunks(diff);
	const coverage = detect_divergences({ source: content, ours, prettier, diff, hunks, language });
	return { diff, hunks, coverage };
}

function print_usage(): void {
	console.log(`
Usage: deno task corpus:compare:format <path> [options]
       deno task corpus:compare:format --all [options]

Arguments:
  path              Directory to scan for source files
  --all             Compare all default corpus repos

Options:
  --filter <lang>   Only compare files of this language (svelte, typescript, css)
  --limit <n>       Limit to first n files per language
  --verbose         Show each file as it's processed
  --exit-on-first   Stop after finding the first mismatch or error (shows diff)
  --safety-only     Only check for safety violations (data loss), skip formatting comparison
  --explain         Show detected divergence patterns for each difference
  --summary         Compact output (no diffs, just file lists with brief descriptions)
  --strict          Fail on any difference (disable divergence detection)
  --audit-patterns  Show per-pattern corpus coverage with sample diffs for spot-checking
  --json            Emit a single JSON report to stdout (stats + safety/partial/
                    unknown/error file lists); all human/progress output → stderr
  --help            Show this help message

Examples:
  deno task corpus:compare:format ~/dev/my-project
  deno task corpus:compare:format ~/dev/my-project --filter svelte
  deno task corpus:compare:format ~/dev/my-project --limit 50 --verbose
  deno task corpus:compare:format ~/dev/my-project --exit-on-first
  deno task corpus:compare:format ~/dev/my-project --safety-only
  deno task corpus:compare:format ~/dev/my-project --explain
  deno task corpus:compare:format --all --audit-patterns
  deno task corpus:compare:format --all --summary
`);
}

// --- JSON output (--json) ----------------------------------------------------
//
// In --json mode all human/progress output is routed to stderr and stdout
// carries a single buffered JSON object: a `stats` block plus per-file lists for
// the statuses that need attention (safety / partial / unknown / errors). The
// `results` map already holds every non-match result in memory for the end-of-run
// report, so there's nothing to stream — and excluding `match` records and full
// diffs keeps the object small regardless of corpus size.

/** Flatten a {@link LanguageStats} to the count shape used in JSON. */
function stats_to_counts(s: LanguageStats) {
	return {
		total: s.total,
		match: s.match,
		known: s.known_divergence,
		partial: s.partial_divergence,
		unknown: s.unknown_diff,
		safety: s.safety_violation,
		errors: s.errors,
		expected_errors: s.expected_errors,
	};
}

/** Build the `stats` block: per-language counts plus a summed total. */
function build_stats_block(stats: Map<Language, LanguageStats>) {
	const languages: Record<string, ReturnType<typeof stats_to_counts>> = {};
	const totals = empty_stats();
	for (const lang of LANGUAGES) {
		const s = stats.get(lang)!;
		if (s.total === 0) continue;
		languages[lang] = stats_to_counts(s);
		totals.total += s.total;
		totals.match += s.match;
		totals.known_divergence += s.known_divergence;
		totals.partial_divergence += s.partial_divergence;
		totals.unknown_diff += s.unknown_diff;
		totals.safety_violation += s.safety_violation;
		totals.expected_errors += s.expected_errors;
		totals.errors += s.errors;
	}
	return { languages, total: stats_to_counts(totals) };
}

/** One per-file entry in a JSON status list. Detail varies by status; diffs are never included. */
function json_file_entry(
	r: CompareResult,
	lang: Language,
	base_path: string,
): Record<string, unknown> {
	const base = { path: rel_path(r.path, base_path), language: lang, bytes: r.bytes };
	switch (r.status) {
		case 'safety_violation':
			return { ...base, violations: r.safety_violations ?? [] };
		case 'partial_divergence':
			return { ...base, patterns: r.coverage?.matches.map((m) => m.pattern) ?? [] };
		case 'unknown_diff':
			// One-line summary instead of the full diff, to keep the object bounded.
			return { ...base, diff_summary: get_diff_summary(r.prettier ?? '', r.ours ?? '') };
		case 'expected_error':
			return { ...base, error: r.error, expected_reason: r.expected_reason };
		case 'error':
			return { ...base, error: r.error };
		default:
			return base;
	}
}

/**
 * Build the single buffered JSON report: stats + per-file lists for the statuses
 * worth inspecting. `match` and `known_divergence` are excluded (their counts
 * live in `stats`); full diffs are excluded (unknowns carry a one-line summary).
 */
function build_json_report(
	results: Map<Language, CompareResult[]>,
	stats: Map<Language, LanguageStats>,
	base_path: string,
): Record<string, unknown> {
	const by_status = (status: CompareResult['status']) =>
		LANGUAGES.flatMap((lang) =>
			results.get(lang)!.filter((r) => r.status === status).map((r) =>
				json_file_entry(r, lang, base_path)
			)
		);
	return {
		stats: build_stats_block(stats),
		safety: by_status('safety_violation'),
		partial: by_status('partial_divergence'),
		unknown: by_status('unknown_diff'),
		errors: by_status('error'),
		expected_errors: by_status('expected_error'),
	};
}

/**
 * The tool's entry, importable by the `conformance.ts` driver (which passes
 * `['--all']`); the CLI wrapper at the bottom feeds it the real argv. Failure
 * semantics are process-level (`Deno.exit(1)` at each gate), matching the old
 * `&&`-chain aggregate exactly.
 */
export async function run_corpus_compare_format(argv: string[] = Deno.args): Promise<void> {
	const parsed = args_parse(argv_parse(argv), CorpusCompareArgs);
	if (!parsed.success) {
		console.error(z.prettifyError(parsed.error));
		print_usage();
		Deno.exit(1);
	}
	const args = parsed.data;

	if (args.help) {
		print_usage();
		return;
	}

	const use_all_repos = args.all;
	const path = args._[0]?.toString();

	if (!path && !use_all_repos) {
		console.error('Error: No path provided (use --all for all repos)\n');
		print_usage();
		Deno.exit(1);
	}

	const base_path = resolve_compare_base_path(path, use_all_repos);
	const filter_lang = parse_language_filter(args.filter);

	const limit = args.limit;
	const verbose = args.verbose;
	const exit_on_first = args['exit-on-first'];
	const safety_only = args['safety-only'];
	const explain = args.explain;
	const strict = args.strict;
	const audit_patterns = args['audit-patterns'];
	const summary = args.summary;
	const json_mode = args.json;

	if (json_mode) redirect_logs_to_stderr();

	if (use_all_repos) {
		console.log('Comparing: All default corpus repos');
	} else {
		console.log(`Comparing: ${base_path}`);
	}
	if (filter_lang) console.log(`Filter: ${filter_lang} only`);
	if (limit) console.log(`Limit: ${limit} files per language`);
	if (safety_only) console.log(`Mode: safety-only (checking for data loss)`);
	if (strict) console.log(`Mode: strict (no divergence detection)`);
	if (explain) console.log(`Mode: explain (show divergence patterns)`);
	if (audit_patterns) console.log(`Mode: audit-patterns (per-pattern coverage report)`);
	if (summary) console.log(`Mode: summary (compact output, no diffs)`);
	console.log();

	const loader = create_compare_loader(use_all_repos, base_path);
	const { canonical, native } = await init_compare_implementations();
	// Content-addressed prettier-output cache (lib/prettier_cache.ts) — the
	// dominant cost here is prettier over ~6k mostly-unchanged files. Keyed on
	// content + routing + options + the canonical-5 pins + PRETTIER_DEBUG;
	// success-only. TSV_PRETTIER_CACHE=0 disables.
	const prettier_cache = canonical.enable_format_cache();

	// Initialize per-language tracking
	const results: Map<Language, CompareResult[]> = new Map();
	const stats: Map<Language, LanguageStats> = new Map();
	// The reproducible subset (version-pinned framework + prettier suites; the
	// `file.reproducible` files). The format count pins gate on THIS, not the
	// aggregate `stats` — live dev-repo churn can't shift a pinned count. Only the
	// gated fields (match/unknown_diff/partial_divergence) are meaningful here; the
	// live divergences are reported as a non-gating WARN (aggregate minus repro).
	const repro_stats: Map<Language, LanguageStats> = new Map();

	for (const lang of LANGUAGES) {
		results.set(lang, []);
		stats.set(lang, empty_stats());
		repro_stats.set(lang, empty_stats());
	}

	// Track divergence pattern counts
	const divergence_counts: Map<string, number> = new Map();

	// Track per-pattern file claims for audit (pattern → file samples with hunk info)
	interface PatternAuditEntry {
		path: string;
		hunk_indices: number[];
		hunk_preview: string; // first hunk's first changed line
	}
	const pattern_audit_map: Map<string, PatternAuditEntry[]> = new Map();

	/** Record a pattern match into the audit map */
	function record_audit_entry(
		pattern_name: string,
		file_path: string,
		hunk_indices: number[],
		hunks: DiffHunk[],
	): void {
		const entries = pattern_audit_map.get(pattern_name) ?? [];
		const first_hunk = hunks[hunk_indices[0]];
		const preview = (first_hunk?.added_lines[0] || first_hunk?.removed_lines[0] || '')
			.trim().slice(0, 60);
		entries.push({
			path: rel_path(file_path, base_path),
			hunk_indices,
			hunk_preview: preview,
		});
		pattern_audit_map.set(pattern_name, entries);
	}

	/** Tally a file's explained pattern matches into the global counts (and audit map). */
	function tally_patterns(
		coverage: HunkCoverageResult,
		file_path: string,
		hunks: DiffHunk[],
	): void {
		for (const d of coverage.matches) {
			divergence_counts.set(d.pattern, (divergence_counts.get(d.pattern) || 0) + 1);
			if (audit_patterns) record_audit_entry(d.pattern, file_path, d.hunk_indices, hunks);
		}
	}

	// Track per-language file counts for filtering/limiting
	const lang_counts: Record<Language, number> = { svelte: 0, typescript: 0, css: 0 };

	// Stream and process files (file content is GC'd after each iteration)
	for await (const file of loader.stream(verbose ? console.log : () => {})) {
		const lang = file.language;
		if (filter_lang && lang !== filter_lang) continue;
		if (limit && lang_counts[lang] >= limit) continue;
		lang_counts[lang]++;

		const lang_stats = stats.get(lang)!;
		// The reproducible-subset accumulator — non-null only for a version-pinned file.
		// INVARIANT: mirror every GATED-count increment below (match / unknown_diff /
		// partial_divergence) into `repro` too. The pins read `repro_stats`; the aggregate
		// `stats` counts all files. (known/safety/errors are aggregate-only — SAFETY gates
		// every file — so they deliberately don't mirror.)
		const repro = file.reproducible ? repro_stats.get(lang)! : null;
		const lang_results = results.get(lang)!;
		lang_stats.total++;

		if (verbose) {
			console.log(`  ${file.path}`);
		}

		let should_exit = false;
		try {
			// Format with both
			const ours = native.format(file.content, lang);
			// Pass the real path so the oracle routes `.js` → babel (preserves JSDoc casts)
			// vs `.ts` → typescript — see `canonical.format_async`.
			const prettier = await canonical.format_async(file.content, lang, file.path);

			// Prettier is the source of truth for the differential safety check
			// below. An empty format of NON-empty source means the in-process
			// prettier malfunctioned, not that the file is empty — and trusting it
			// is actively dangerous: an empty `prettier` inflates `prettier_excess`
			// to the whole source, so the differential would CANCEL (mask) any real
			// loss in `ours` (see the `safety_test.ts` "prettier-empty masks a real
			// loss" case). Whitespace-only output is semantically empty to the
			// differential (`count_semantic_chars` strips whitespace), so it masks
			// exactly the same way — hence `.trim()`. Error out so the miss is
			// surfaced rather than silently suppressing a safety verdict.
			if (prettier.trim() === '' && file.content.trim() !== '') {
				throw new Error(
					'prettier returned empty output for non-empty source (prettier miss — safety verdict unreliable)',
				);
			}

			// Safety check FIRST (always) — DIFFERENTIAL against prettier.
			// Prettier is the source of truth, so any char transformation prettier
			// ALSO performs (redundant leading `|` removal, number normalization,
			// CSS keyword lowercasing, shorthand collapsing foo={foo} → {foo}) is a
			// legitimate normalization, not data loss. The differential reports only
			// the loss/addition OUR output incurs BEYOND prettier — false positives
			// from shared normalizations cancel out. This subsumes the old
			// `ours !== prettier` guard: when ours === prettier the real set is empty.
			const safety_violations = check_safety_vs_prettier(file.content, ours, prettier);
			if (safety_violations.length > 0) {
				// A real (non-shared) violation remains. Still run divergence detection
				// so intentional divergences that legitimately differ from prettier —
				// BOM stripping (prettier keeps the BOM), self-closing normalization —
				// reclassify as known rather than SAFETY.
				//
				// Safety guarantee: if we lose content that prettier preserves, the
				// differential keeps it AND it surfaces as an unexplained diff hunk →
				// classification stays SAFETY.
				//
				// The downgrade keys on `safety_vouched`, NOT `all_explained`. The latter is
				// a set-cover over hunk indices, so it cannot distinguish the hunk that
				// carried the flagged characters from one that merely sits in the same file:
				// on `prettier/tests/format/html/tags/tags.html` the whole 9-char delta lives
				// in a self-closing-tag hunk, yet two unrelated whitespace hunks were equally
				// load-bearing for the downgrade — a change to the pattern claiming THOSE
				// would have flipped the file into this gated bucket with no formatter change.
				// `safety_vouched` requires every char-risky hunk to be claimed by a pattern
				// that declares `may_alter_char_frequency`, restoring the causal link.
				const { hunks, coverage } = run_detection(file.content, ours, prettier, lang);

				if (coverage.safety_vouched) {
					// Safety violations are fully explained by known divergence patterns
					lang_stats.known_divergence++;
					lang_results.push({
						path: file.path,
						bytes: file.bytes,
						status: 'known_divergence',
						coverage,
					});
					tally_patterns(coverage, file.path, hunks);
				} else {
					// Unexplained safety violations — before recording data loss,
					// self-verify: re-run the native format and require byte-identity
					// with the first run. The historical Deno-FFI heisenbug fabricated
					// SAFETY by corrupting OURS non-deterministically (see
					// lib/ffi.ts + CLAUDE.md §Known Issues); a deterministic re-run is
					// the discriminator (a real loss reproduces, corruption doesn't).
					// On divergence, throw — the process's FFI layer can't be trusted
					// for a verdict on this file, so it must surface as a loud error,
					// never as a fabricated (or vanished) SAFETY finding.
					const ours_verify = native.format(file.content, lang);
					if (ours_verify !== ours) {
						throw new Error(
							'native format nondeterminism: two runs on identical input differ (FFI corruption — safety verdict unreliable)',
						);
					}
					lang_stats.safety_violation++;
					lang_results.push({
						path: file.path,
						bytes: file.bytes,
						status: 'safety_violation',
						safety_violations,
						coverage: coverage.classification !== 'none_explained' ? coverage : undefined,
					});
					if (exit_on_first) {
						const rel = rel_path(file.path, base_path);
						console.log(`\nSafety violation: ${rel}`);
						for (const v of safety_violations) {
							console.log(`  ${v.type}: ${v.summary}`);
						}
						should_exit = true;
					}
				}
			} else if (safety_only) {
				// In safety-only mode, we're done after safety check passes
				lang_stats.match++;
				if (repro) repro.match++;
			} else if (ours === prettier) {
				// Exact match — only counted, not stored
				lang_stats.match++;
				if (repro) repro.match++;
			} else {
				// Difference detected
				const rel = rel_path(file.path, base_path);
				if (strict) {
					// Strict mode: any difference is a failure
					lang_stats.unknown_diff++;
					if (repro) repro.unknown_diff++;
					lang_results.push({
						path: file.path,
						bytes: file.bytes,
						status: 'unknown_diff',
						ours,
						prettier,
					});
					if (exit_on_first) {
						console.log(`\nDifference (strict mode): ${rel}`);
						should_exit = true;
					}
				} else {
					// Detect known divergence patterns (hunk-aware)
					const { diff, hunks, coverage } = run_detection(file.content, ours, prettier, lang);

					if (coverage.classification === 'all_explained') {
						// All hunks explained by known patterns
						lang_stats.known_divergence++;
						lang_results.push({
							path: file.path,
							bytes: file.bytes,
							status: 'known_divergence',
							coverage,
						});
						tally_patterns(coverage, file.path, hunks);
					} else if (coverage.classification === 'partial') {
						// Some hunks explained, some not
						lang_stats.partial_divergence++;
						if (repro) repro.partial_divergence++;
						lang_results.push({
							path: file.path,
							bytes: file.bytes,
							status: 'partial_divergence',
							coverage,
						});
						tally_patterns(coverage, file.path, hunks);
					} else {
						// No hunks explained - unknown difference
						lang_stats.unknown_diff++;
						if (repro) repro.unknown_diff++;
						lang_results.push({
							path: file.path,
							bytes: file.bytes,
							status: 'unknown_diff',
							ours,
							prettier,
							coverage,
						});
						if (exit_on_first) {
							console.log(`\nUnknown difference: ${rel}`);
							console.log('─'.repeat(70));
							const removals = diff.filter((d) => d.type === 'remove').length;
							const additions = diff.filter((d) => d.type === 'add').length;
							console.log(
								`Diff: \x1b[31m- Prettier\x1b[0m → \x1b[32m+ Ours\x1b[0m  (${removals} prettier-only, ${additions} ours-only)`,
							);
							console.log('');
							for (const line of format_diff_for_terminal(filter_diff_context(diff))) {
								console.log(line);
							}
							should_exit = true;
						}
					}
				}
			}
		} catch (e) {
			const error_msg = e instanceof Error ? e.message : String(e);
			const expected_check = check_expected_error(file.content, lang);
			if (expected_check.expected) {
				lang_stats.expected_errors++;
				lang_results.push({
					path: file.path,
					bytes: file.bytes,
					status: 'expected_error',
					error: error_msg,
					expected_reason: expected_check.pattern!.reason,
				});
			} else {
				lang_stats.errors++;
				lang_results.push({
					path: file.path,
					bytes: file.bytes,
					status: 'error',
					error: error_msg,
				});
				if (exit_on_first) {
					console.log(`\nError: ${rel_path(file.path, base_path)}`);
					console.log(`  ${error_msg}`);
					should_exit = true;
				}
			}
		}

		if (should_exit) {
			canonical.dispose();
			native.dispose();
			Deno.exit(1);
		}
	}

	const total_processed = Object.values(lang_counts).reduce((a, b) => a + b, 0);
	if (total_processed === 0) {
		// An empty scope is a failed comparison run, not a pass — an existing-but-
		// source-empty path (typo, moved src/) must not read as green.
		console.log('No files found — nothing was compared.');
		if (json_mode) emit_json_stdout(build_json_report(results, stats, base_path));
		canonical.dispose();
		native.dispose();
		Deno.exit(1);
	}

	const counts = LANGUAGES.map((lang) => `${lang_counts[lang]} ${lang}`).join(', ');
	console.log(`\nProcessed: ${total_processed} files (${counts})\n`);

	// Print results
	console.log('Results:');

	let total_match = 0;
	let total_known_divergence = 0;
	let total_partial_divergence = 0;
	let total_unknown_diff = 0;
	let total_safety_violation = 0;
	let total_expected_errors = 0;
	let total_errors = 0;
	let total_count = 0;

	/** Build the detail parts array for a stats row */
	function build_detail_parts(s: LanguageStats): string[] {
		const parts: string[] = [];
		if (s.known_divergence > 0) parts.push(`${s.known_divergence} known`);
		if (s.partial_divergence > 0) parts.push(`\x1b[33m${s.partial_divergence} partial\x1b[0m`);
		if (s.unknown_diff > 0) parts.push(`${s.unknown_diff} unknown`);
		if (s.safety_violation > 0) parts.push(`\x1b[31m${s.safety_violation} SAFETY\x1b[0m`);
		if (s.errors > 0) parts.push(`${s.errors} errors`);
		if (s.expected_errors > 0) parts.push(`\x1b[2m${s.expected_errors} expected errors\x1b[0m`);
		return parts;
	}

	for (const lang of LANGUAGES) {
		const s = stats.get(lang)!;
		if (s.total === 0) continue;

		total_match += s.match;
		total_known_divergence += s.known_divergence;
		total_partial_divergence += s.partial_divergence;
		total_unknown_diff += s.unknown_diff;
		total_safety_violation += s.safety_violation;
		total_expected_errors += s.expected_errors;
		total_errors += s.errors;
		total_count += s.total;

		const pct = s.total > 0 ? ((s.match / s.total) * 100).toFixed(1) : '100.0';
		const match_str = `${s.match}/${s.total} match (${pct}%)`.padEnd(24);
		const parts = build_detail_parts(s);
		const detail_str = parts.length > 0 ? parts.join(' | ') : 'all match';
		console.log(`  ${lang.padEnd(12)} ${match_str} | ${detail_str}`);
	}

	if (total_count > 0) {
		console.log('  ' + '─'.repeat(72));
		const pct = total_count > 0 ? ((total_match / total_count) * 100).toFixed(1) : '100.0';
		const match_str = `${total_match}/${total_count} match (${pct}%)`.padEnd(24);

		const totals: LanguageStats = {
			total: total_count,
			match: total_match,
			known_divergence: total_known_divergence,
			partial_divergence: total_partial_divergence,
			unknown_diff: total_unknown_diff,
			safety_violation: total_safety_violation,
			expected_errors: total_expected_errors,
			errors: total_errors,
		};
		const parts = build_detail_parts(totals);

		const detail_str = parts.length > 0 ? parts.join(' | ') : 'all match';
		console.log(`  ${'total'.padEnd(12)} ${match_str} | ${detail_str}`);
	}

	// Emit the buffered JSON report now — before the FAIL Deno.exit calls below,
	// so it is always written on normal completion regardless of the final exit
	// code. NOT written by the earlier-exiting paths: --exit-on-first (the
	// `should_exit` block above) bails mid-stream, and init/validation failures
	// exit before `results`/`stats` exist. The top-level `main().catch` emits a
	// minimal error-shaped JSON report for those (and any other) rejections.
	if (json_mode) emit_json_stdout(build_json_report(results, stats, base_path));

	// Show divergence pattern breakdown if any detected
	if (divergence_counts.size > 0 && (explain || verbose)) {
		console.log('\nKnown Divergence Patterns:');
		const sorted = [...divergence_counts.entries()].sort((a, b) => b[1] - a[1]);
		for (const [pattern, count] of sorted) {
			console.log(`  ${pattern}: ${count} files`);
		}
	}

	// Show per-pattern audit report with sample diffs
	if (audit_patterns && pattern_audit_map.size > 0) {
		console.log('\nPattern Audit (per-pattern corpus coverage)');
		console.log('─'.repeat(70));

		const sorted = [...pattern_audit_map.entries()].sort((a, b) => b[1].length - a[1].length);
		for (const [pattern, entries] of sorted) {
			console.log(`\n${pattern}: ${entries.length} files`);
			const samples = entries.slice(0, 3);
			for (const sample of samples) {
				const hunk_str = sample.hunk_indices.length === 1
					? `hunk ${sample.hunk_indices[0]}`
					: `hunks ${sample.hunk_indices.join(',')}`;
				console.log(`  ${sample.path} (${hunk_str})`);
				if (sample.hunk_preview) {
					console.log(`    "${sample.hunk_preview}"`);
				}
			}
			if (entries.length > 3) {
				console.log(`  ... and ${entries.length - 3} more`);
			}
		}
	}

	// Show safety violations (CRITICAL)
	const all_safety_violations = results_by_status(results, 'safety_violation');

	if (all_safety_violations.length > 0) {
		console.log(`\n\x1b[31mSAFETY VIOLATIONS (${all_safety_violations.length} files):\x1b[0m`);
		for (const r of all_safety_violations) {
			console.log(`  ${rel_path(r.path, base_path)}`);
			for (const v of r.safety_violations!) {
				console.log(`    - ${v.type}: ${v.summary}`);
			}
		}
	}

	// Show partial divergences (some hunks unexplained)
	const all_partial = results_by_status(results, 'partial_divergence')
		.sort((a, b) => a.bytes - b.bytes);

	// Show unknown differences (needs investigation)
	const all_unknown = results_by_status(results, 'unknown_diff')
		.sort((a, b) => a.bytes - b.bytes);

	// Default: show unexplained diffs (partial hunks + unknown files)
	// --summary: compact output without diffs
	if (summary) {
		// Compact partial divergence listing
		if (all_partial.length > 0) {
			console.log(
				`\nPartial Divergences (${all_partial.length} files):`,
			);
			for (const r of all_partial.slice(0, 10)) {
				const coverage = r.coverage!;
				const patterns = coverage.matches.map((d) => d.pattern).join(', ');
				console.log(
					`  ${
						rel_path(r.path, base_path)
					}: ${patterns} (${coverage.unexplained_hunks.length} unexplained hunks)`,
				);
			}
			if (all_partial.length > 10) {
				console.log(`  ... and ${all_partial.length - 10} more`);
			}
		}

		// Compact unknown differences listing
		if (all_unknown.length > 0) {
			console.log(
				`\nUnknown Differences (${all_unknown.length} files, needs investigation):`,
			);
			for (const r of all_unknown.slice(0, 10)) {
				const size_str = format_bytes(r.bytes);
				const diff_summary = get_diff_summary(r.prettier!, r.ours!);
				console.log(`  ${rel_path(r.path, base_path)} (${size_str})`);
				console.log(`    ${diff_summary}`);
			}
			if (all_unknown.length > 10) {
				console.log(`  ... and ${all_unknown.length - 10} more`);
			}
		}
	} else {
		// Default: show all unexplained diffs
		const total_unexplained_files = all_partial.length + all_unknown.length;
		if (total_unexplained_files > 0) {
			console.log(
				`\nUnexplained Differences (${all_partial.length} partial + ${all_unknown.length} unknown = ${total_unexplained_files} files):`,
			);
		}

		// Partial files: show only unexplained hunks with diffs
		if (all_partial.length > 0) {
			console.log(`\n${'─'.repeat(70)}`);
			console.log(`Partial files (${all_partial.length} — unexplained hunks only):`);
			for (const r of all_partial) {
				const coverage = r.coverage!;
				const patterns = coverage.matches.map((d) => d.pattern).join(', ');
				const explained_count = coverage.explained_hunks.size;
				const total_hunks = coverage.hunks.length;
				console.log(`\n  ${rel_path(r.path, base_path)}:`);
				console.log(
					`    explained ${explained_count}/${total_hunks} hunks: ${patterns}`,
				);
				for (const idx of coverage.unexplained_hunks) {
					const hunk = coverage.hunks[idx];
					const ours_label = hunk.ours_range ? `ours:${hunk.ours_range.start}` : '';
					const prettier_label = hunk.prettier_range ? `prettier:${hunk.prettier_range.start}` : '';
					console.log(
						`    \x1b[33mhunk ${idx}\x1b[0m: @@ ${ours_label} / ${prettier_label} @@`,
					);
					for (const line of format_diff_for_terminal(hunk.lines)) {
						console.log(`      ${line}`);
					}
				}
			}
		}

		// Unknown files: show full diffs
		if (all_unknown.length > 0) {
			console.log(`\n${'─'.repeat(70)}`);
			console.log(`Unknown files (${all_unknown.length} — full diffs):`);
			for (const r of all_unknown) {
				const diff = diff_lines(r.prettier!, r.ours!);
				const removals = diff.filter((d) => d.type === 'remove').length;
				const additions = diff.filter((d) => d.type === 'add').length;
				console.log(`\n  ${rel_path(r.path, base_path)} (${format_bytes(r.bytes)}):`);
				console.log(
					`    \x1b[31m-${removals} prettier-only\x1b[0m, \x1b[32m+${additions} ours-only\x1b[0m`,
				);
				for (const line of format_diff_for_terminal(filter_diff_context(diff))) {
					console.log(`      ${line}`);
				}
			}
		}
	}

	// Show known divergences with explanations if --explain
	if (explain) {
		const all_known = results_by_status(results, 'known_divergence');

		if (all_known.length > 0) {
			console.log(`\nKnown Divergences (${all_known.length} files):`);
			for (const r of all_known) {
				const patterns = r.coverage!.matches.map((d) => d.pattern).join(', ');
				console.log(`  ${rel_path(r.path, base_path)}: ${patterns}`);
			}
		}
	}

	// Show errors (unexpected only)
	const all_errors = results_by_status(results, 'error').sort((a, b) => a.bytes - b.bytes);

	if (all_errors.length > 0) {
		console.log(`\nErrors (${all_errors.length} files):`);
		for (const r of all_errors.slice(0, 3)) {
			const size_str = format_bytes(r.bytes);
			console.log(`  ${rel_path(r.path, base_path)} (${size_str}): ${r.error?.slice(0, 80)}`);
		}
		if (all_errors.length > 3) {
			console.log(`  ... and ${all_errors.length - 3} more`);
		}
	}

	// Show expected errors (dimmed, verbose/explain only for details)
	const all_expected_errors = results_by_status(results, 'expected_error')
		.sort((a, b) => a.bytes - b.bytes);

	if (all_expected_errors.length > 0 && (verbose || explain)) {
		console.log(`\n\x1b[2mExpected Errors (${all_expected_errors.length} files):\x1b[0m`);
		for (const r of all_expected_errors) {
			console.log(`\x1b[2m  ${rel_path(r.path, base_path)}: ${r.expected_reason}\x1b[0m`);
		}
	}

	// Disclose cache participation so a triage reader knows which side was live
	// (cached hits remove the prettier-side flake from repeat runs; the tsv/FFI
	// side is always live).
	if (prettier_cache) console.log(`\n${prettier_cache.stats()}`);

	// Pinned counts (--all only — see lib/gate_counts.ts). The count pins gate on
	// the REPRODUCIBLE subset (`repro_stats` — version-pinned framework + prettier
	// suites, tracked by GATE_CHECKOUT_COMMITS + pins:audit), so live dev-repo churn
	// can't shift them: per-language MINIMUM `match` (a drop = a formatter/oracle
	// collapse in pinned code) + EXACT `unknown`/`partial` (up = a new unexplained
	// divergence to fix/catalog; down = backlog shrank, re-pin). The live `real`
	// repos are a NON-GATING WARN below (their divergences are reported, never fail
	// — unversioned working trees); SAFETY still gates over EVERY file (see below).
	// A single-run trip can be the FFI/sidecar heisenbug — confirm on the single
	// repo before treating as real.
	if (use_all_repos) {
		// Non-gating WARN: divergences in the live dev repos (aggregate − reproducible).
		const live_warn = LANGUAGES.flatMap((lang) => {
			const a = stats.get(lang)!;
			const r = repro_stats.get(lang)!;
			const u = a.unknown_diff - r.unknown_diff;
			const p = a.partial_divergence - r.partial_divergence;
			return u > 0 || p > 0 ? [`${lang} ${u} unknown / ${p} partial`] : [];
		});
		if (live_warn.length > 0) {
			console.log(
				`\n\x1b[33mWARN: live dev-repo divergences (non-gating) — ${live_warn.join('; ')}. ` +
					`Unversioned working trees, so not pinned; triage a specific one with ` +
					`\`corpus:compare:format <repo>\`. SAFETY still gates these.\x1b[0m`,
			);
		}

		const pin = (lang: Language) => repro_stats.get(lang)!;
		const pin_failures = [
			...LANGUAGES.filter((lang) => pin(lang).match < CORPUS_FORMAT_MATCH_MIN[lang]).map(
				(lang) => `${lang} match ${pin(lang).match} < pinned minimum ${CORPUS_FORMAT_MATCH_MIN[lang]}`,
			),
			...LANGUAGES.filter((lang) => pin(lang).unknown_diff !== CORPUS_FORMAT_UNKNOWN_PIN[lang]).map(
				(lang) => `${lang} unknown ${pin(lang).unknown_diff} ≠ pinned ${CORPUS_FORMAT_UNKNOWN_PIN[lang]}`,
			),
			...LANGUAGES.filter(
				(lang) => pin(lang).partial_divergence !== CORPUS_FORMAT_PARTIAL_PIN[lang],
			).map(
				(lang) => `${lang} partial ${pin(lang).partial_divergence} ≠ pinned ${CORPUS_FORMAT_PARTIAL_PIN[lang]}`,
			),
		];
		if (pin_failures.length > 0) {
			console.log(
				`\n\x1b[31mFAIL: pinned counts (reproducible subset) — ${pin_failures.join('; ')}. ` +
					`If deliberate, re-pin in lib/gate_counts.ts.\x1b[0m`,
			);
			canonical.dispose();
			native.dispose();
			Deno.exit(1);
		}
	}

	// Final status
	console.log();
	if (total_safety_violation > 0) {
		console.log(
			`\x1b[31mFAIL: ${total_safety_violation} safety violations (data loss detected)\x1b[0m`,
		);
		canonical.dispose();
		native.dispose();
		Deno.exit(1);
	} else if ((total_unknown_diff > 0 || total_partial_divergence > 0) && strict) {
		const issues = total_unknown_diff + total_partial_divergence;
		console.log(`\x1b[31mFAIL: ${issues} unexplained differences (strict mode)\x1b[0m`);
		canonical.dispose();
		native.dispose();
		Deno.exit(1);
	} else if (total_unknown_diff > 0 || total_partial_divergence > 0) {
		const parts: string[] = [];
		if (total_unknown_diff > 0) parts.push(`${total_unknown_diff} unknown`);
		if (total_partial_divergence > 0) parts.push(`${total_partial_divergence} partial`);
		console.log(
			`\x1b[33mWARN: ${parts.join(', ')} differences (may need investigation)\x1b[0m`,
		);
	} else if (
		total_errors > 0 &&
		total_match + total_known_divergence + total_expected_errors === 0
	) {
		// Every file errored — a systemic failure (sidecar/FFI down, wrong corpus),
		// NOT a graded run. Without this floor the SAFETY differential never runs
		// on any file and the gate would WARN + exit 0 having compared nothing.
		console.log(
			`\x1b[31mFAIL: all ${total_errors} files errored — nothing was compared (systemic sidecar/FFI failure?)\x1b[0m`,
		);
		canonical.dispose();
		native.dispose();
		Deno.exit(1);
	} else if (total_errors > 0) {
		console.log(`\x1b[33mWARN: ${total_errors} errors occurred\x1b[0m`);
	} else {
		console.log('\x1b[32mPASS: No safety violations or unknown differences\x1b[0m');
	}

	// Cleanup
	canonical.dispose();
	native.dispose();
}

/**
 * Build a minimal but valid, `JSON.parse`-able report for the failure path:
 * an empty `stats` block (no languages, zeroed totals) plus an `error` field.
 * Keeps the documented contract that `--json` always writes a parseable
 * document to stdout, even when `main()` rejects before the normal emit.
 */
function build_error_json_report(message: string): Record<string, unknown> {
	const empty_lang_stats: Map<Language, LanguageStats> = new Map(
		LANGUAGES.map((lang) => [lang, empty_stats()]),
	);
	return {
		stats: build_stats_block(empty_lang_stats),
		safety: [],
		partial: [],
		unknown: [],
		errors: [],
		expected_errors: [],
		error: message,
	};
}

if (import.meta.main) {
	run_compare_main(run_corpus_compare_format, CorpusCompareArgs, build_error_json_report);
}

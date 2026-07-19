/**
 * Divergence Detection Audit Script
 *
 * Cross-references detection patterns against conformance_prettier.md to verify:
 * 1. Every documented divergence has a pattern that claims to detect it
 * 2. Every pattern fixture reference points to a documented divergence
 *
 * Usage:
 *   deno task divergence:audit
 *   deno task divergence:audit --json
 */

import process from 'node:process';

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import { z } from 'zod';

import { format_audit_report, generate_audit_report } from './lib/divergence/mod.ts';

const AuditArgs = z.object({
	json: z.boolean().default(false),
	help: z.boolean().default(false).meta({ aliases: ['h'] }),
});

function print_usage(): void {
	console.log(`
Usage: deno task divergence:audit [options]

Options:
  --json    Output results as JSON
  --help    Show this help message

Examples:
  deno task divergence:audit           # Human-readable report
  deno task divergence:audit --json    # Machine-readable JSON
`);
}

async function main(): Promise<void> {
	const parsed = args_parse(argv_parse(process.argv.slice(2)), AuditArgs);
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

	const report = await generate_audit_report();

	if (args.json) {
		console.log(JSON.stringify(report, null, 2));
	} else {
		console.log(format_audit_report(report));
		console.log('');

		// A broken listing is a hard error — it points at nothing on disk. (The
		// gate for it is `fixture_coverage_test`, in `deno task check`; reported
		// here too since this audit is where a listing is read.)
		if (report.missing_pattern_fixtures.length > 0) {
			const count = report.missing_pattern_fixtures.reduce((n, m) => n + m.fixtures.length, 0);
			console.log(`\x1b[31mFAIL: ${count} pattern fixture listing(s) point at no directory\x1b[0m`);
			Deno.exit(1);
		}

		// Exit non-zero on genuine detection gaps only. Coverage is partial by
		// design (docs/divergence_detector.md §Traceability), so this is a WARN
		// that reports work, not a broken invariant — but it now names fixtures
		// no detector can see, rather than fixtures nobody wrote down.
		if (report.stats.total_undetected > 0 || report.stats.total_partial > 0) {
			console.log(
				`\x1b[33mWARN: ${report.stats.total_undetected} documented divergences are detected ` +
					`by no pattern; ${report.stats.total_partial} more leave hunks unexplained\x1b[0m`,
			);
			if (report.stats.total_unlisted_but_explained > 0) {
				console.log(
					`      (${report.stats.total_unlisted_but_explained} more are fully explained but ` +
						`unlisted in fixtures[] — bookkeeping, not a gap)`,
				);
			}
			Deno.exit(1);
		}
		console.log('\x1b[32mPASS: every gradeable documented divergence is fully explained\x1b[0m');
	}
}

main();

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

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.js';
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

		// Exit with error if coverage is below 100%
		if (report.stats.total_uncovered > 0) {
			console.log('');
			console.log(
				`\x1b[33mWARN: ${report.stats.total_uncovered} documented divergences have no pattern coverage\x1b[0m`,
			);
			Deno.exit(1);
		} else {
			console.log('');
			console.log('\x1b[32mPASS: All documented divergences have pattern coverage\x1b[0m');
		}
	}
}

main();

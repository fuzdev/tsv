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

import { parseArgs } from '@std/cli/parse-args';
import { formatAuditReport, generateAuditReport } from './lib/divergence/mod.ts';

interface Args {
	json?: boolean;
	help?: boolean;
}

function printUsage(): void {
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
	const args = parseArgs(Deno.args, {
		boolean: ['json', 'help'],
		alias: { h: 'help' },
	}) as Args;

	if (args.help) {
		printUsage();
		return;
	}

	const report = await generateAuditReport();

	if (args.json) {
		console.log(JSON.stringify(report, null, 2));
	} else {
		console.log(formatAuditReport(report));

		// Exit with error if coverage is below 100%
		if (report.stats.totalUncovered > 0) {
			console.log('');
			console.log(
				`\x1b[33mWARN: ${report.stats.totalUncovered} documented divergences have no pattern coverage\x1b[0m`,
			);
			Deno.exit(1);
		} else {
			console.log('');
			console.log('\x1b[32mPASS: All documented divergences have pattern coverage\x1b[0m');
		}
	}
}

main();

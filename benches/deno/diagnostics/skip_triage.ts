/**
 * Diagnostic: which corpus files does tsv fail to parse, and does the
 * canonical parser (svelte/compiler / acorn-typescript) handle them?
 *
 * Mirrors bench.ts's untimed pre-flight, but parse-only and with cross-impl
 * asymmetry buckets (tsv-fails-canonical-ok = real gaps). Not wired
 * into `deno task` — run ad hoc (full JSON to stdout, summary to stderr):
 *   deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys \
 *     benches/deno/diagnostics/skip_triage.ts 2>/dev/null > /tmp/triage.json
 */

import { DevReposLoader, group_by_language } from '../lib/corpus.ts';
import { init_implementations } from '../lib/implementations.ts';
import type { Language } from '../lib/types.ts';

const [files, impls] = await Promise.all([
	new DevReposLoader().load((m) => console.error(m)),
	init_implementations({ logger: (m) => console.error(m) }),
]);
const by_language = group_by_language(files);
if (!impls.native) throw new Error('native FFI not built');

const langs: Language[] = ['svelte', 'typescript', 'css'];

interface Buckets {
	tsv_fails_canonical_ok: { path: string; error: string }[];
	canonical_fails_tsv_ok: { path: string; error: string }[];
	both_fail: { path: string; tsv_error: string; canonical_error: string }[];
}

const report: Record<string, Buckets> = {};

for (const lang of langs) {
	const buckets: Buckets = {
		tsv_fails_canonical_ok: [],
		canonical_fails_tsv_ok: [],
		both_fail: [],
	};
	for (const f of by_language[lang]) {
		let tsv_err: string | null = null;
		let canon_err: string | null = null;
		try {
			// parse_internal suffices: only throw/no-throw is read, and it skips
			// the full JSON materialization (Rust to_string + FFI copy + JS
			// JSON.parse) — same $lang::parse + error surface in tsv_ffi
			impls.native!.parse_internal(f.content, lang);
		} catch (e) {
			tsv_err = String(e instanceof Error ? e.message : e).split('\n')[0];
		}
		try {
			impls.canonical.parse(f.content, lang);
		} catch (e) {
			canon_err = String(e instanceof Error ? e.message : e).split('\n')[0];
		}
		if (tsv_err && !canon_err) {
			buckets.tsv_fails_canonical_ok.push({ path: f.path, error: tsv_err });
		} else if (!tsv_err && canon_err) {
			buckets.canonical_fails_tsv_ok.push({ path: f.path, error: canon_err });
		} else if (tsv_err && canon_err) {
			buckets.both_fail.push({ path: f.path, tsv_error: tsv_err, canonical_error: canon_err });
		}
	}
	report[lang] = buckets;
}

// Summary to stderr, full JSON to stdout.
for (const lang of langs) {
	const b = report[lang];
	console.error(
		`\n${lang}: tsv-fails-canonical-ok=${b.tsv_fails_canonical_ok.length}  ` +
			`canonical-fails-tsv-ok=${b.canonical_fails_tsv_ok.length}  ` +
			`both-fail=${b.both_fail.length}`,
	);
}
console.log(JSON.stringify(report, null, 2));

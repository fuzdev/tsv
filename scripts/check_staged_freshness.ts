/**
 * Freshness guard for the STAGED npm-package artifacts the `:run` test tasks
 * consume — the staged-package sibling of
 * `benches/js/lib/check_artifact_freshness.ts` (the bench/corpus `:run` guard).
 *
 * `deno task test:npm[:parse|:all]` and `deno task test:napi:npm` build first,
 * so what they test is fresh by construction. Their `:run` variants
 * deliberately skip that build — the path for iterating on the test harness —
 * at the risk of silently testing a STALE staging: the incident this guard
 * exists for was `crates/tsv_napi/pkg` holding a native `tsv_cli` binary from
 * before a behavior fix, which `test:napi:npm:run` would have green-tested
 * without a word. `scripts/validate_artifacts.ts` is the third consumer, for
 * the same reason with no `:run` split at all — it never builds, so a bare
 * `deno task validate:artifacts` grades whatever `pkg/` happens to hold.
 *
 * Two postures, one grading. `assert_staged_fresh` ABORTS, which is right over
 * an artifact the caller's own task builds. `staged_staleness` grades one
 * artifact and hands the reason back, for a caller whose right answer is to
 * SKIP — a check reaching into a package it cannot rebuild, where failing
 * would only be telling the operator to run someone else's build (that is
 * `test_napi_npm.ts` over `@fuzdev/tsv_wasm`). Both read the same mtimes, so a
 * skip and an abort never disagree about what is stale.
 *
 * Staleness here has two lags — the `target/` build behind the sources, and the
 * staged copy behind the build — and comparing the staged file's mtime directly
 * against the SOURCES catches both with one check.
 *
 * Each check names the crates and/or individual files that feed one staged
 * artifact; when crates are named, the workspace `Cargo.toml` + `Cargo.lock`
 * are considered too (dependency bumps rebuild the artifact). A missing staged
 * file is always fatal; `BENCH_STALE_OK=1` — the same escape hatch as the
 * bench guard, deliberately one knob for every mtime guard — downgrades a
 * stale (present-but-older) one to a warning. What no mtime check can see is a
 * toolchain change; after one, rebuild once via the build-first task.
 */

import { stat } from 'node:fs/promises';
import { env, exit } from 'node:process';
import { fileURLToPath } from 'node:url';

import { fmt_mtime, newest_source_mtime } from '../benches/js/lib/check_artifact_freshness.ts';

const ROOT = fileURLToPath(new URL('..', import.meta.url));

/** One staged artifact to check against the sources that feed it. */
export interface StagedCheck {
	/** Human-readable label used in messages. */
	label: string;
	/** Repo-root-relative path to the staged file. */
	staged: string;
	/** Crates under `crates/` whose Rust sources compile into it (may be empty). */
	crates: string[];
	/** Repo-root-relative individual source files (staging scripts, copied JS). */
	files: string[];
	/** Command that restages this artifact, surfaced in the error message. */
	rebuild: string;
}

/** Why one staged artifact is not fresh — `staged_staleness`'s verdict. */
export interface StagedStaleness {
	/** The staged file is absent, not merely old. Always fatal, never overridable. */
	missing: boolean;
	/** One line naming what is stale (or absent); carries no restage hint. */
	reason: string;
}

/**
 * Grade ONE staged artifact against the sources feeding it and return why it
 * is not fresh, without aborting — the seam for a caller that wants to SKIP
 * rather than fail, which is the right posture over a package the caller does
 * not itself build. `assert_staged_fresh` is this over a list, plus the abort.
 */
export async function staged_staleness(check: StagedCheck): Promise<StagedStaleness | undefined> {
	let staged_ms: number;
	try {
		staged_ms = (await stat(`${ROOT}${check.staged}`)).mtimeMs;
	} catch {
		return { missing: true, reason: `${check.label}: not staged — ${check.staged}` };
	}

	let newest = { ms: 0, path: '' };
	if (check.crates.length > 0) {
		newest = { ...(await newest_source_mtime(check.crates)) };
		for (const workspace_file of ['Cargo.toml', 'Cargo.lock']) {
			try {
				const st = await stat(`${ROOT}${workspace_file}`);
				if (st.mtimeMs > newest.ms) newest = { ms: st.mtimeMs, path: workspace_file };
			} catch {
				// no lockfile (fresh clone pre-build) — the crate sources govern
			}
		}
	}
	for (const file of check.files) {
		try {
			const st = await stat(`${ROOT}${file}`);
			if (st.mtimeMs > newest.ms) newest = { ms: st.mtimeMs, path: file };
		} catch {
			// a named source that doesn't exist can't out-date the staging
		}
	}

	// Strict `<` so an artifact staged in the same second as its source passes.
	if (staged_ms < newest.ms) {
		return {
			missing: false,
			reason:
				`${check.label}: staged ${fmt_mtime(staged_ms)}, ` +
				`but ${newest.path} changed ${fmt_mtime(newest.ms)}`
		};
	}
	return undefined;
}

/**
 * Abort (exit 1) when any staged artifact is missing or older than a source
 * feeding it; see the module doc for the escape hatch. Returns normally when
 * everything is fresh (or staleness was downgraded to a warning).
 */
export async function assert_staged_fresh(checks: readonly StagedCheck[]): Promise<void> {
	const stale_ok = env.BENCH_STALE_OK === '1';
	const findings: string[] = [];
	let missing = false;

	for (const check of checks) {
		const staleness = await staged_staleness(check);
		if (!staleness) continue;
		if (staleness.missing) missing = true;
		findings.push(`  • ${staleness.reason}`);
		findings.push(`      restage: ${check.rebuild}`);
	}

	if (findings.length === 0) return;

	const fatal = missing || !stale_ok;
	console.error(
		[
			'',
			fatal
				? '✗ Stale staged package artifacts — refusing to grade outdated code.'
				: '⚠ Stale staged package artifacts (BENCH_STALE_OK=1 — grading anyway).',
			...findings,
			...(fatal
				? [
						'',
						'  Run the build-first task rather than the `:run` variant, run the restage',
						'  command(s) above, or set BENCH_STALE_OK=1 to override (stale only —',
						'  a missing staged file is always fatal).'
					]
				: []),
			''
		].join('\n')
	);
	if (fatal) exit(1);
}

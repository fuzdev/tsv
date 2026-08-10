/**
 * Platform-triple detection for `@fuzdev/tsv` — the one copy shared by the
 * loader (`index.js`, which resolves the platform package's addon) and the
 * `tsv` bin dispatcher (`bin.js`, which resolves the same package's native
 * CLI binary without loading the addon).
 */

import { readdirSync } from 'node:fs';

/**
 * Whether this Linux runs musl (Alpine). The cheap probe runs first: a musl
 * system carries its loader in `/lib`, so a miss (the common glibc case) is
 * one readdir (~0.1 ms) — where the diagnostic report costs ~2.4 ms, paid on
 * every loader import and every `tsv` bin dispatch. Only a hit consults the
 * report to rule out a glibc system that merely has musl installed:
 * `glibcVersionRuntime` is trusted only positively (Bun's report may be
 * partial). Verdict-equivalent to the report-first ordering.
 */
export const is_musl = () => {
	let has_musl_loader;
	try {
		has_musl_loader = readdirSync('/lib').some((f) => f.startsWith('ld-musl-'));
	} catch {
		has_musl_loader = false;
	}
	if (!has_musl_loader) return false;
	const report =
		typeof process.report?.getReport === 'function' ? process.report.getReport() : null;
	return !report?.header?.glibcVersionRuntime;
};

export const platform_triple = () => {
	const { platform, arch } = process;
	if (platform === 'linux') return `linux-${arch}-${is_musl() ? 'musl' : 'gnu'}`;
	return `${platform}-${arch}`;
};

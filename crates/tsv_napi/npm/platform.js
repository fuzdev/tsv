/**
 * Platform-triple detection for `@fuzdev/tsv` — the one copy shared by the
 * loader (`index.js`, which resolves the platform package's addon) and the
 * `tsv` bin dispatcher (`bin.js`, which resolves the same package's native
 * CLI binary without loading the addon).
 */

import { readdirSync, readFileSync } from 'node:fs';

/**
 * The three facts `is_musl` reads, each behind a function so a verdict never
 * pays for a probe it did not need. A bag rather than three module-level
 * functions because that is what lets the DECISION be exercised over hosts this
 * machine is not (`scripts/test_napi_npm.ts` drives the matrix).
 */
export const LIBC_PROBES = {
	/**
	 * Whether `/lib` carries a musl loader (`ld-musl-<arch>.so.1`). ~0.08 ms, and
	 * false on a stock glibc system — which is why it is asked first.
	 */
	musl_loader: () => {
		try {
			return readdirSync('/lib').some((f) => f.startsWith('ld-musl-'));
		} catch {
			return false;
		}
	},
	/**
	 * The libc mapped into THIS process, read from its own memory map (~0.16 ms):
	 * `'musl'`, `'gnu'`, or `undefined` when neither is mapped (no `/proc`, or a
	 * statically linked host binary). Exact where the filesystem is only
	 * suggestive — a glibc system that merely has musl installed carries the
	 * loader but maps glibc.
	 */
	mapped_libc: () => {
		let maps;
		try {
			maps = readFileSync('/proc/self/maps', 'utf-8');
		} catch {
			return undefined;
		}
		if (/\/(?:ld-musl-|libc\.musl-)/.test(maps)) return 'musl';
		if (/\/(?:ld-linux|libc\.so\.6)/.test(maps)) return 'gnu';
		return undefined;
	},
	/**
	 * The glibc version the diagnostic report names, or `undefined`. Costs
	 * ~1.8 ms — the reason it is asked last rather than first — and is trusted
	 * only POSITIVELY: a runtime may ship no `process.report` at all, or a header
	 * without the field.
	 */
	reported_glibc: () => {
		const report =
			typeof process.report?.getReport === 'function' ? process.report.getReport() : null;
		return report?.header?.glibcVersionRuntime;
	}
};

/**
 * Whether this Linux runs musl (Alpine) — which platform package's `libc` this
 * host wants.
 *
 * Three probes, cheapest first, each able to end it:
 *
 * 1. No musl loader in `/lib`: a stock glibc system, and the common case. One
 *    readdir, no further questions.
 * 2. The libc actually mapped into this process. Authoritative for either
 *    verdict, and the answer to the case the filesystem cannot settle — a glibc
 *    host that merely has musl INSTALLED carries the loader from step 1 and maps
 *    glibc.
 * 3. Only when the map was unreadable or named neither: the diagnostic report's
 *    `glibcVersionRuntime`, trusted positively. A missing or partial report is
 *    the case that used to resolve to musl on such a host, which is a package
 *    the platform never installed and a load error pointing at the wrong remedy.
 *
 * With every probe silent the loader from step 1 stands: a musl system whose
 * `/proc` this process cannot read is still a musl system.
 */
export const is_musl = (probes = LIBC_PROBES) => {
	if (!probes.musl_loader()) return false;
	const mapped = probes.mapped_libc();
	if (mapped !== undefined) return mapped === 'musl';
	return !probes.reported_glibc();
};

export const platform_triple = () => {
	const { platform, arch } = process;
	if (platform === 'linux') return `linux-${arch}-${is_musl() ? 'musl' : 'gnu'}`;
	return `${platform}-${arch}`;
};

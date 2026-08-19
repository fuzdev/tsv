/**
 * Tiny cross-runtime helpers for the bench harness.
 *
 * The harness runs under both Deno and Node from one shared codebase: shared
 * modules use `node:` builtins directly (Deno supports them) and avoid `Deno.*`
 * / `@std/*`. This module is the handful of spots where the two runtimes' own
 * identifiers differ — platform naming and the runtime label itself — plus the
 * `runtime` tag stamped onto every report row so a reader never has to guess
 * what produced a number. The genuinely runtime-specific code (the native FFI
 * vs N-API loader) lives in `ffi.ts` / `napi.ts`, not here.
 */

import { cpus } from 'node:os';
import {
	arch as node_arch,
	platform as node_platform,
	versions as node_versions
} from 'node:process';

/** The JS runtime executing the harness. Stamped on every report row. */
export type Runtime = 'deno' | 'node' | 'bun';

/** Rust-target-style OS name (`linux` | `darwin` | `windows`), matching the
 * old `Deno.build.os` values used for binary/library path construction. */
export type Os = 'linux' | 'darwin' | 'windows';

/** Detect the current runtime. `typeof` on the undeclared globals is safe in
 * every runtime (no ReferenceError). Order matters: Bun also defines a partial
 * `process`, and Deno defines neither `Bun` nor a Node-only marker, so probe the
 * distinctive globals first. */
export function current_runtime(): Runtime {
	if (typeof (globalThis as { Deno?: unknown }).Deno !== 'undefined') return 'deno';
	if (typeof (globalThis as { Bun?: unknown }).Bun !== 'undefined') return 'bun';
	return 'node';
}

/** Normalize `process.platform` to the `Deno.build.os` vocabulary the binary
 * path / library-name logic was written against. */
export function current_os(): Os {
	switch (node_platform) {
		case 'win32':
			return 'windows';
		case 'darwin':
			return 'darwin';
		default:
			// linux (and the rare *bsd, treated as linux for our .so naming)
			return 'linux';
	}
}

/** Normalize `process.arch` to the Rust-target vocabulary (`x86_64` /
 * `aarch64`) that matched `Deno.build.arch`, used to locate per-platform npm
 * native bindings. Falls through verbatim for anything unmapped. */
export function current_arch(): string {
	switch (node_arch) {
		case 'x64':
			return 'x86_64';
		case 'arm64':
			return 'aarch64';
		default:
			return node_arch;
	}
}

/** Platform-shaped filename of a Rust cdylib built from `crate_name` —
 * `libtsv_ffi.so` / `libtsv_ffi.dylib` / `tsv_ffi.dll`. The single home for
 * the prefix/extension mapping shared by the FFI and N-API loaders, the
 * binary-size table, the doctor's artifact checks, and the N-API boundary
 * test. */
export function native_library_filename(crate_name: string): string {
	const os = current_os();
	const ext = os === 'darwin' ? 'dylib' : os === 'windows' ? 'dll' : 'so';
	const prefix = os === 'windows' ? '' : 'lib';
	return `${prefix}${crate_name}.${ext}`;
}

/**
 * The wasm-pack target directory this runtime's bundle lives under — `deno` under
 * Deno, `nodejs` under Node and Bun (which reuses the Node artifacts). Names a
 * path segment: `crates/tsv_wasm/pkg/<variant>/<target>/`.
 *
 * The sibling of `native_library_filename` above, and here for the same reason:
 * the LOADER (`wasm.ts`) and the freshness GUARD (`check_artifact_freshness.ts`)
 * each resolve a path under this segment, and a run that guards one bundle while
 * measuring another is the exact failure the guard exists to prevent — which a
 * second spelling of the conditional is all it would take.
 */
export function wasm_target(): 'deno' | 'nodejs' {
	return current_runtime() === 'deno' ? 'deno' : 'nodejs';
}

/**
 * The machine that produced a report — the stable hardware identity plus the
 * runtime's own version. The bench's throughput numbers are machine-relative,
 * so this travels with them (a top-level `machine` on every
 * `report.<runtime>.json`): without it a report copied to the site, or diffed
 * against an older one, can't tell a code change from a different box.
 * Deliberately excludes hostname (the reports are published) and volatile
 * fields (free memory, load average, live CPU frequency) that would churn the
 * committed report every run — only the stable hardware identity belongs here.
 *
 * ⚠️ A WIRE shape, not merely a producer-side struct, and it is read back by a
 * consumer this module never sees: `compose_reports.ts` imports this same type to
 * describe SIBLING reports on disk, which routinely predate the current producer
 * (the composer folds whatever reports exist, and flags the vintage spread rather
 * than refusing it). Every field below is therefore asserted over data written by
 * an older bench. That holds today because the composer reads only the four fields
 * this type shipped with, at `REPORT_SCHEMA_VERSION` 7.
 *
 * So ADDING a field here is a schema change, not a local one. Bump
 * `REPORT_SCHEMA_VERSION` (bench.ts) and mark the new field `Since version N`, the
 * way `Baseline`'s own fields are marked — the nesting is what makes it easy to
 * miss, since `Baseline.machine` itself doesn't change. Any composer code reading
 * the new field owes it the per-field optionality the composer already spells out
 * by hand for `UnavailableImpl.rows`; the shared type cannot express "present only
 * from version N" for both sides at once.
 */
export interface Machine {
	/** `os.cpus()[0].model` — the stable CPU identifier (e.g. `AMD Ryzen 9 7950X`). */
	cpu_model: string;
	/** Rust-target OS name (`linux` | `darwin` | `windows`), from `current_os()`. */
	os: Os;
	/** Rust-target arch (`x86_64` | `aarch64` | …), from `current_arch()`. */
	arch: string;
	/**
	 * The runtime's own version — `Deno.version.deno` under Deno,
	 * `process.versions.{node,bun}` under Node/Bun. Distinct per sibling report;
	 * the hardware fields above are identical across them (same box).
	 */
	runtime_version: string;
}

/** The executing runtime's own version string. Deno exposes it on
 * `Deno.version.deno`; Node and Bun both expose it at `process.versions[runtime]`
 * (`.node` / `.bun`). Returns `'unknown'` if unavailable. */
export function runtime_version(): string {
	const runtime = current_runtime();
	if (runtime === 'deno') {
		return (
			(globalThis as { Deno?: { version?: { deno?: string } } }).Deno?.version?.deno ?? 'unknown'
		);
	}
	return node_versions[runtime] ?? 'unknown';
}

/** Snapshot the current machine — the hardware identity (CPU model, OS, arch)
 * plus the runtime version. Stable across runs on one box, so it adds no churn
 * to the committed report; it changes exactly when the box or runtime does. */
export function current_machine(): Machine {
	return {
		cpu_model: cpus()[0]?.model ?? 'unknown',
		os: current_os(),
		arch: current_arch(),
		runtime_version: runtime_version()
	};
}

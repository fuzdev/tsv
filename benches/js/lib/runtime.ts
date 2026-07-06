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

import { arch as node_arch, platform as node_platform } from 'node:process';

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

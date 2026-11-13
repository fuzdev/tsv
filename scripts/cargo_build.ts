/**
 * `cargo build`, then date every final artifact it reports at now — the build-side
 * half of the mtime guards (`benches/js/lib/check_artifact_freshness.ts`,
 * `scripts/check_staged_freshness.ts`), and what `npm run build:stamped` runs.
 *
 * Those guards read an artifact's mtime as WHEN IT WAS LAST BUILT and compare it
 * against the mtimes of everything feeding it. Cargo does not promise that reading. It
 * keys a manifest and the lockfile by CONTENT, not by date, so a `Cargo.lock` or
 * `Cargo.toml` rewritten to something it has already built — a version bump restored
 * and applied again, a `git reset`, a branch switched away and back — leaves every
 * unit fresh: `cargo build` is a no-op that re-links the cached output, original date
 * and all. The guard then refuses an artifact that is current, and the rebuild command
 * it names cannot clear it.
 *
 * Stamping makes the mtime mean the one thing every guard needs it to — when cargo
 * last CONFIRMED this artifact current — whether that took a compile or a no-op. A
 * `:run` task still sees a real staleness: nothing here runs unless a build does.
 *
 * Its reach is cargo's: the cargo program, run in the checkout with its output passed
 * through, and the stamp, which reaches beneath `target/` only (`StampDir`): an artifact
 * cargo reports anywhere else (a `CARGO_TARGET_DIR` outside the checkout) is refused, not
 * dated.
 *
 * Its entry is `scripts/roots/cargo_build.ts`, whose module doc says why this module and
 * everything it imports stay on `node:*` and relative modules, never a package.
 *
 * Only a LEAF artifact is stamped: a `bin`'s executable, a `cdylib`'s dynamic library.
 * Never a `lib` — cargo dates a unit's dependents against its outputs, so an rlib
 * dated forward rebuilds everything above it on the next build — which is why a target
 * that is a `cdylib` AND an rlib (`tsv_wasm`) is left alone: its two files are one
 * unit's outputs. A `bin` has dependents too, its own package's integration tests,
 * which is harmless in the profiles the stamped tasks build (none runs tests) and the
 * reason the dev-profile `npm run build` stays a plain `cargo build`.
 *
 * Which files those are is cargo's to say (`--message-format=json-render-diagnostics`:
 * artifact messages on stdout, diagnostics still rendered to stderr), so no task names
 * an output path here and the per-platform spellings stay cargo's.
 *
 * Usage (every argument is passed to `cargo build`):
 *   node scripts/roots/cargo_build.ts -p tsv_ffi --release
 *
 * Exits with cargo's own code, `128 + N` for a cargo ended by signal `N`; a failed build stamps
 * nothing.
 *
 * @module
 */

import { isAbsolute, relative, sep } from 'node:path';

import { lines_of } from '../benches/js/lib/text_lines.ts';
import { signal_exit_code } from './exit_code.ts';

/** A dynamic library as cargo names it on each platform — not a Windows import
 * library (`.dll.lib`) or debug file beside it. */
const DYNAMIC_LIBRARY = /\.(?:so|dylib|dll)$/;

/**
 * The files one line of cargo's JSON output names for stamping: a `bin` target's
 * executable, a `cdylib`-only target's dynamic library, and nothing for any other
 * message (the module doc says why a `lib` in the kind list rules a target out).
 */
export function stamp_paths(message: unknown): string[] {
	if (typeof message !== 'object' || message === null) return [];
	const { reason, target, filenames, executable } = message as {
		reason?: unknown;
		target?: { kind?: unknown };
		filenames?: unknown;
		executable?: unknown;
	};
	if (reason !== 'compiler-artifact') return [];
	const kind = target?.kind;
	if (!Array.isArray(kind) || kind.length !== 1) return [];
	if (kind[0] === 'bin') return typeof executable === 'string' ? [executable] : [];
	if (kind[0] === 'cdylib' && Array.isArray(filenames)) {
		return filenames.filter(
			(name): name is string => typeof name === 'string' && DYNAMIC_LIBRARY.test(name)
		);
	}
	return [];
}

/** The times a stamp sets, in milliseconds since the epoch. */
export interface StampTimes {
	mtime_ms: number;
	atime_ms?: number;
}

/**
 * The directory the stamp may date files beneath: `path` is where cargo's reported paths are
 * read against, and `set_times` dates one file by its path relative to it.
 */
export interface StampDir {
	readonly path: string;
	set_times(path: string, times: StampTimes): Promise<void>;
}

/** How a cargo run ended: its exit code, or the name of the signal that ended it. */
export type CargoExit = { kind: 'exited'; code: number } | { kind: 'signaled'; signal: string };

/** A started cargo: its stdout, and how it ended. */
export interface CargoChild {
	stdout: AsyncIterable<Uint8Array>;
	/** Rejects when cargo could not be started. */
	closed: Promise<CargoExit>;
}

/** What `run_cargo_build` reaches. */
export interface CargoBuildDeps {
	/** Starts `cargo <args>` in `cwd`, its stdin and stderr inherited and its stdout read here. */
	start_cargo: (args: ReadonlyArray<string>, cwd: string) => CargoChild;
	/** The checkout's path, cargo's working directory. */
	checkout: string;
	/** Where the artifacts are stamped: the checkout's `target/`. */
	target: StampDir;
	/** Writes a line of cargo's stdout that isn't one of its JSON messages. */
	print: (line: string) => void;
}

/**
 * `path` relative to `dir`, `/`-separated, or `null` when it is `dir` itself or not beneath
 * it — the artifact the stamp refuses to reach.
 */
export function to_stamp_path(dir: string, path: string): string | null {
	const rel = relative(dir, path);
	if (rel === '' || isAbsolute(rel) || rel === '..' || rel.startsWith(`..${sep}`)) return null;
	return rel.split(sep).join('/');
}

/** The exit code for a cargo that ended as `exit`: its own, or `128 + N` for signal `N`. */
export function to_exit_code(exit: CargoExit): number {
	return exit.kind === 'exited' ? exit.code : signal_exit_code(exit.signal);
}

/**
 * `cargo build --message-format=json-render-diagnostics <args>`, then every leaf artifact it
 * reports dated at now (module doc).
 * @returns cargo's exit code, `128 + N` for a cargo ended by signal `N`
 * @throws `Error` - for an artifact beneath no `target`, after a build that succeeded
 */
export async function run_cargo_build(
	deps: CargoBuildDeps,
	args: ReadonlyArray<string>
): Promise<number> {
	const child = deps.start_cargo(
		['build', '--message-format=json-render-diagnostics', ...args],
		deps.checkout
	);

	const paths = new Set<string>();
	for await (const line of lines_of(child.stdout)) {
		let message: unknown;
		try {
			message = JSON.parse(line);
		} catch {
			// not one of cargo's messages — pass it through rather than swallow it
			deps.print(line);
			continue;
		}
		for (const path of stamp_paths(message)) paths.add(path);
	}

	const code = to_exit_code(await child.closed);
	if (code !== 0) return code;

	const stamps: string[] = [];
	for (const path of paths) {
		const stamp = to_stamp_path(deps.target.path, path);
		if (stamp === null) {
			throw new Error(`refusing to stamp ${path}: it is not beneath ${deps.target.path}`);
		}
		stamps.push(stamp);
	}
	const now = Date.now();
	for (const stamp of stamps) await deps.target.set_times(stamp, { mtime_ms: now, atime_ms: now });
	return 0;
}

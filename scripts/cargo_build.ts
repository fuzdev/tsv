/**
 * `cargo build`, then date every final artifact it reports at now — the build-side
 * half of the mtime guards (`benches/js/lib/check_artifact_freshness.ts`,
 * `scripts/check_staged_freshness.ts`), and what `deno task build:stamped` runs.
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
 * Only a LEAF artifact is stamped: a `bin`'s executable, a `cdylib`'s dynamic library.
 * Never a `lib` — cargo dates a unit's dependents against its outputs, so an rlib
 * dated forward rebuilds everything above it on the next build — which is why a target
 * that is a `cdylib` AND an rlib (`tsv_wasm`) is left alone: its two files are one
 * unit's outputs. A `bin` has dependents too, its own package's integration tests,
 * which is harmless in the profiles the stamped tasks build (none runs tests) and the
 * reason the dev-profile `deno task build` stays a plain `cargo build`.
 *
 * Which files those are is cargo's to say (`--message-format=json-render-diagnostics`:
 * artifact messages on stdout, diagnostics still rendered to stderr), so no task names
 * an output path here and the per-platform spellings stay cargo's.
 *
 * Usage (every argument is passed to `cargo build`):
 *   deno run --allow-run=cargo --allow-write=target scripts/cargo_build.ts -p tsv_ffi --release
 *
 * Exits with cargo's own code; a failed build stamps nothing.
 *
 * @module
 */

import { lines_of } from '../benches/js/lib/text_lines.ts';

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

async function main(): Promise<void> {
	const child = new Deno.Command('cargo', {
		args: ['build', '--message-format=json-render-diagnostics', ...Deno.args],
		stdin: 'inherit',
		stdout: 'piped',
		stderr: 'inherit'
	}).spawn();

	const paths = new Set<string>();
	for await (const line of lines_of(child.stdout)) {
		let message: unknown;
		try {
			message = JSON.parse(line);
		} catch {
			// not one of cargo's messages — pass it through rather than swallow it
			console.log(line);
			continue;
		}
		for (const path of stamp_paths(message)) paths.add(path);
	}

	const status = await child.status;
	if (!status.success) Deno.exit(status.code || 1);

	const now = new Date();
	for (const path of paths) Deno.utimeSync(path, now, now);
}

if (import.meta.main) await main();

/**
 * rsvelte-fmt implementation wrapper (native binary, one process per file)
 *
 * `@rsvelte/fmt` is the other Rust-native Svelte formatter. It is the only
 * alternative in this harness with **no in-process API**: the npm package is a
 * Node launcher that `spawnSync`s a platform binary, and nothing in the
 * `@rsvelte` scope exposes formatting to JS (the sibling
 * `@rsvelte/vite-plugin-svelte-native` N-API addon is the *compiler* — `compile`
 * / `parse` / `svelte2tsx`, no format export). So it can only be driven as a
 * subprocess, which is why its row is **coverage-only**: it is measured for what
 * it accepts, never timed. See `implementations.ts` (`coverage_only`) and
 * docs/benchmarks.md §Coverage-only rows.
 *
 * Two decisions keep the coverage number meaningful:
 *
 * - **The platform binary is invoked directly**, not the published
 *   `bin/rsvelte-fmt` launcher — the launcher adds a Node cold start per file,
 *   which measures npm packaging rather than rsvelte. Nothing here depends on
 *   the launcher's work: it exists to locate this same binary and to forward an
 *   `--oxfmt-bin`, and `.svelte` never needs oxfmt (embedded `<style>` bodies go
 *   through the binary's own `oxc_formatter_css`, verified with oxfmt absent).
 * - **Svelte only.** rsvelte-fmt's `.ts`/`.js` path is `oxc_formatter` and its
 *   CSS path `oxc_formatter_css` — the same engine as the existing `oxfmt` row,
 *   by its own `--no-native-js` / `--no-native-css` escape-hatch docs. Svelte is
 *   the only language where it is a distinct engine.
 */

import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { BaseImplementation, type Language } from './types.ts';
import type { RsvelteVersions } from './versions.ts';

/**
 * Map the current platform/arch to the `@rsvelte/fmt-<triple>` package that
 * carries the prebuilt binary, or `null` when rsvelte publishes none for it.
 * Mirrors `@rsvelte/fmt`'s own `lib/resolve.cjs`, minus its musl detection —
 * rsvelte publishes no musl package, so a musl host has no binary either way.
 */
function platform_package(): string | null {
	const { platform, arch } = process;
	if (platform === 'darwin') {
		if (arch === 'arm64') return '@rsvelte/fmt-darwin-arm64';
		if (arch === 'x64') return '@rsvelte/fmt-darwin-x64';
	} else if (platform === 'linux') {
		if (arch === 'x64') return '@rsvelte/fmt-linux-x64-gnu';
		if (arch === 'arm64') return '@rsvelte/fmt-linux-arm64-gnu';
	} else if (platform === 'win32') {
		if (arch === 'x64') return '@rsvelte/fmt-win32-x64-msvc';
	}
	return null;
}

/**
 * Absolute path to the prebuilt binary in the harness `node_modules`, or `null`
 * when this platform has no package or npm skipped the optional dependency.
 *
 * Resolved by path rather than `require.resolve` so the module stays portable
 * across all three runtimes (the same reason `lib/binary_sizes.ts` reads
 * `node_modules` directly).
 */
export function rsvelte_binary_path(): string | null {
	const pkg = platform_package();
	if (pkg === null) return null;
	const node_modules = fileURLToPath(new URL('../node_modules', import.meta.url));
	const bin_name = process.platform === 'win32' ? 'rsvelte-fmt.exe' : 'rsvelte-fmt';
	const path = join(node_modules, ...pkg.split('/'), bin_name);
	return existsSync(path) ? path : null;
}

/**
 * rsvelte-fmt implementation, driven as a subprocess.
 *
 * Supports:
 * - Format: Svelte only (see the module doc)
 * - Parse: unsupported — the CLI emits formatted source, never an AST
 */
export class RsvelteImplementation extends BaseImplementation {
	readonly versions: RsvelteVersions;
	private _bin: string | null = null;
	private _config_path: string | null = null;

	/** It's a formatter CLI — it emits source, never an AST. */
	readonly parse_languages: ReadonlyArray<Language> = [];
	readonly format_languages: ReadonlyArray<Language> = ['svelte'];

	constructor(versions: RsvelteVersions) {
		super();
		this.versions = versions;
	}

	// Nothing to await — unlike the WASM impls there's no module to load, just a
	// path to resolve and a binary to probe. `async` stays because the registry
	// awaits every impl's `init` uniformly (`TsvImplementation`).
	// deno-lint-ignore require-await
	async init(): Promise<void> {
		const bin = rsvelte_binary_path();
		if (bin === null) {
			throw new Error(
				`no @rsvelte/fmt binary for ${process.platform}-${process.arch} in the harness node_modules`
			);
		}

		// Match the prettier/tsv config — width 100, tabs, single quotes, no
		// trailing commas — so the accept check does the same layout work every
		// other row does. `--print-width` / `--use-tabs` are flags, but the quote
		// and trailing-comma keys exist only in `.oxfmtrc`, so the remaining two
		// live in the committed `../rsvelte.oxfmtrc.json` (deliberately NOT named
		// `.oxfmtrc.json`: that exact name is what oxfmt and rsvelte-fmt DISCOVER
		// by walking up from the working directory, so a real one in the repo would
		// reach into every oxfmt-backed row).
		//
		// Passing `--config` explicitly is itself load-bearing: without it
		// rsvelte-fmt runs that upward discovery, and a stray `.oxfmtrc.json`
		// anywhere above the repo would silently restyle this row. See
		// docs/benchmarks.md §Coverage-only rows.
		const config_path = fileURLToPath(new URL('../rsvelte.oxfmtrc.json', import.meta.url));
		if (!existsSync(config_path)) {
			throw new Error(`missing rsvelte-fmt bench config at ${config_path}`);
		}
		this._config_path = config_path;
		this._bin = bin;

		// Assert the binary actually runs before the registry reports it available —
		// a package present but unexecutable (pnpm's mode-0644 pack, a foreign
		// arch) would otherwise surface as every file failing, reading as 0%
		// coverage rather than a broken setup.
		const probe = spawnSync(bin, ['--version'], { encoding: 'utf8' });
		if (probe.error) throw probe.error;
		if (probe.status !== 0) {
			throw new Error(`rsvelte-fmt --version exited ${probe.status}`);
		}
	}

	parse(_source: string, _language: Language): unknown {
		throw new Error('rsvelte-fmt has no parser: the CLI emits formatted source, never an AST');
	}

	/**
	 * Format one file by spawning the binary with the source on stdin.
	 *
	 * Deliberately synchronous: the caller is the pre-flight accept check, which
	 * is sequential anyway, and `spawnSync` keeps the shape identical across all
	 * three runtimes. This is NEVER on a timed path — see the module doc.
	 */
	format(source: string, language: Language): string {
		if (!this._bin || !this._config_path) {
			throw new Error('rsvelte-fmt not initialized');
		}
		if (!this.supports_format_language(language)) {
			throw new Error(`rsvelte-fmt does not support ${language}`);
		}

		const result = spawnSync(
			this._bin,
			[
				'--stdin',
				'--stdin-filepath',
				'file.svelte',
				'--print-width',
				'100',
				'--use-tabs',
				'--config',
				this._config_path,
				// Keep the run hermetic: the on-disk `<style>` cache would otherwise
				// write state outside the harness and let a stale entry answer for the
				// engine.
				'--no-style-cache'
			],
			{ input: source, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }
		);

		if (result.error) throw result.error;
		// A signal kill (a Rust panic aborting) leaves `status` null — report it as
		// the crash it is rather than letting `status !== 0` describe it vaguely.
		if (result.signal) {
			throw new Error(`rsvelte-fmt terminated by ${result.signal}`);
		}
		if (result.status !== 0) {
			const detail = (result.stderr || '').trim().split('\n')[0] ?? '';
			throw new Error(`rsvelte-fmt exited ${result.status}${detail ? `: ${detail}` : ''}`);
		}
		// Exit 0 with empty output on non-empty input is a silent failure, not a
		// format — the same posture `lib/canonical.ts` takes on an empty prettier
		// result, and the reason a broken binary can't read as full coverage.
		const code = result.stdout ?? '';
		if (source.trim() !== '' && code.trim() === '') {
			throw new Error('rsvelte-fmt returned empty output for non-empty source');
		}
		return code;
	}

	// deno-lint-ignore require-await
	async format_async(source: string, language: Language): Promise<string> {
		return this.format(source, language);
	}

	dispose(): void {
		this._bin = null;
		this._config_path = null;
	}
}

/**
 * What the comparison tools' roots share, not a root itself: the engines they load (tsv's
 * N-API addon at the `corpus` profile and the canonical oracles, in process), the read-only
 * directories they open, `HOME`, the pins files, and the checkout's `tsv_debug`.
 *
 * Each tool's reach is its root's: the corpus loader and the prettier-output cache, which
 * Bun's bench shares, read the corpus and write `benches/js/.cache` through `node:*`; the rest
 * is bound here. A root ends with `exit_with`, not by waiting for the event loop: a tool that
 * threw can leave its `tsv_debug` child running, and the exit ends it.
 *
 * @module
 */

import { resolve } from 'node:path';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { process } from '@fuzdev/gro/process.ts';

import { CHECKOUT_PATH, checkout_program } from '../../../scripts/roots/checkout.ts';
import type { InitCompareImplementations } from '../lib/compare_cli.ts';
import { init_compare_implementations } from '../lib/compare_engines.ts';
import type { PinsFile } from '../lib/corpus_pins.ts';
import { COMPARE_PROFILE } from '../lib/tsv_artifacts.ts';
import {
	TSV_DEBUG_CORPUS,
	TSV_DEBUG_PASS_ENV,
	type TsvDebugProgram
} from '../lib/tsv_debug_server.ts';

/** The engines at `COMPARE_PROFILE`, the profile the tools' tasks build. */
export const init_implementations: InitCompareImplementations = () =>
	init_compare_implementations({ profile: COMPARE_PROFILE });

/**
 * Opens a directory read-only by a path as a tool spells it: relative to the checkout, where
 * `npm run` and `deno task` run, or absolute.
 */
export const open_readonly_dir = async (path: string): Promise<FsReadDir> =>
	(await fs.open_dir(resolve(CHECKOUT_PATH, path))).readonly();

/** `HOME`, the one variable the tools read for a path (`~`, `--all`'s `~/dev`). */
export const read_home = (): string | undefined => process.env.narrow(['HOME']).get('HOME');

/** The pins file `name` in `dir`, read-only. */
export const readonly_pins_file = (dir: FsReadDir, name: string): PinsFile => ({
	read: () => dir.read_text_if_exists(name)
});

/** The pins file `name` in `dir`, which `--update` rewrites. */
export const pins_file = (dir: FsDir, name: string): PinsFile => ({
	read: () => dir.read_text_if_exists(name),
	write: async (text) => {
		await dir.write(name, text);
	}
});

/** Makes the checkout's `target/corpus/tsv_debug`, with the panic hook's backtrace switches. */
export const tsv_debug_opener =
	(checkout: FsDir): (() => TsvDebugProgram) =>
	() =>
		checkout_program(checkout, TSV_DEBUG_CORPUS, { pass_env: TSV_DEBUG_PASS_ENV });

/** Ends the process with `code`. */
export const exit_with = (code: number): never => process.exit(code);

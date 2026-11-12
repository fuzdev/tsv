/**
 * The `node:*` bindings the credentialed release jobs' entries give their libraries
 * (`scripts/roots/publish_napi.ts`, `scripts/roots/release_notes.ts`,
 * `scripts/roots/release_assets.ts`): one run of a program with its output captured, the file
 * operations those libraries take as deps, and each library's whole deps built from them for a
 * checkout. Its own module so a suite reaches every binding an entry hands over without running
 * the entry (`scripts/release/node_host_test.ts`); an entry is left finding its checkout, one
 * call, and setting the exit code.
 *
 * Node's own modules alone and no package: the jobs that run these entries hold the npm token or
 * the repository's write permission, so they install nothing (docs/tasks.md §`publish:napi`).
 *
 * @module
 */

import { Buffer } from 'node:buffer';
import { spawn } from 'node:child_process';
import {
	chmod,
	copyFile,
	lstat,
	mkdir,
	mkdtemp,
	readFile,
	rm,
	stat,
	writeFile
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import process from 'node:process';
import { setTimeout as sleep } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';

import { signal_exit_code } from '../exit_code.ts';
import type { PublishNapiDeps } from '../publish_napi.ts';
import type { ReleaseAssetsDeps } from '../release_assets.ts';
import type { ReleaseNotesDeps } from '../release_notes.ts';

/** How a captured program ended, and what it wrote. */
export interface CapturedRun {
	/** The exit code; `128 + N` for a program ended by signal `N`; `127` for one that can't be
	 * found. */
	code: number;
	/** Everything it wrote to stdout, decoded as UTF-8 (a leading byte-order mark dropped). */
	stdout: string;
	/** Everything it wrote to stderr, decoded the same way; `<program>: command not found`
	 * for one that can't be found. */
	stderr: string;
}

/** Where a captured program runs: the working directory's own unless `cwd` is given. */
export interface CaptureOptions {
	cwd?: string;
}

const decode = (chunks: Array<Buffer>): string => new TextDecoder().decode(Buffer.concat(chunks));

/**
 * Run `program` once, by name through `PATH` and with no shell, with stdin on `/dev/null`, the
 * whole environment, and stdout and stderr captured.
 * @returns how it ended and what it wrote; a program that can't be found (`ENOENT`) is a run
 *   that exited 127, never a throw
 * @throws `Error` - for any other failure to start it
 */
export const node_capture = (
	program: string,
	args: ReadonlyArray<string>,
	options: CaptureOptions = {}
): Promise<CapturedRun> =>
	new Promise((resolve, reject) => {
		const child = spawn(program, [...args], {
			cwd: options.cwd,
			stdio: ['ignore', 'pipe', 'pipe']
		});
		const stdout: Array<Buffer> = [];
		const stderr: Array<Buffer> = [];
		child.stdout.on('data', (chunk: Buffer) => stdout.push(chunk));
		child.stderr.on('data', (chunk: Buffer) => stderr.push(chunk));
		// a spawn failure emits `error` and then `close`; the first settles the run
		child.on('error', (error: Error & { code?: string }) => {
			if (error.code === 'ENOENT') {
				resolve({ code: 127, stdout: '', stderr: `${program}: command not found` });
			} else {
				reject(error);
			}
		});
		child.on('close', (code, signal) => {
			resolve({
				code: code ?? signal_exit_code(signal ?? ''),
				stdout: decode(stdout),
				stderr: decode(stderr)
			});
		});
	});

/** The file operations the release libraries take, over `node:fs`, each by path. */
export const node_release_fs = {
	read_text: (path: string): Promise<string> => readFile(path, 'utf8'),
	read_bytes: async (path: string): Promise<Uint8Array<ArrayBuffer>> =>
		new Uint8Array(await readFile(path)),
	/** Creates or truncates the file, mode `0o666` less the umask. */
	write_text: (path: string, text: string): Promise<void> => writeFile(path, text),
	/** Whether the path, symlinks followed, is a regular file; any error reads as `false`. */
	is_file: async (path: string): Promise<boolean> => {
		try {
			return (await stat(path)).isFile();
		} catch {
			return false;
		}
	},
	/** Whether the path itself, a symlink not followed, is a regular file; any error reads as
	 * `false`. */
	is_regular_file: async (path: string): Promise<boolean> => {
		try {
			return (await lstat(path)).isFile();
		} catch {
			return false;
		}
	},
	/** The file's mode bits, symlinks followed. */
	mode: async (path: string): Promise<number> => (await stat(path)).mode,
	set_mode: (path: string, mode: number): Promise<void> => chmod(path, mode),
	/** The file's size in bytes. */
	size: async (path: string): Promise<number> => (await stat(path)).size,
	/** Creates the directory and any missing parents. */
	make_dir: async (path: string): Promise<void> => {
		await mkdir(path, { recursive: true });
	},
	/** A new directory beneath the system's temp directory, named `prefix` and a random tail.
	 * @returns its absolute path */
	make_temp_dir: (prefix: string): Promise<string> => mkdtemp(join(tmpdir(), prefix)),
	/** Removes the directory and everything beneath it; a missing one is an error. */
	remove_dir: (path: string): Promise<void> => rm(path, { recursive: true }),
	/** Copies the file, keeping its mode. */
	copy_file: (from: string, to: string): Promise<void> => copyFile(from, to)
};

/**
 * The checkout an entry under `scripts/roots/` sits in: two directories above it, never the
 * working directory.
 * @param entry_url - the entry's `import.meta.url`
 */
export const entry_checkout = (entry_url: string): string =>
	resolve(dirname(fileURLToPath(entry_url)), '..', '..');

/** The programs the release entries run, found on `PATH`; a suite names harmless ones. */
export interface ReleasePrograms {
	npm: string;
	tar: string;
}

/** `npm` and `tar`, by name. */
export const RELEASE_PROGRAMS: ReleasePrograms = { npm: 'npm', tar: 'tar' };

/** A line and a newline to stdout. */
const print = (line: string): void => console.log(line);
/** A line and a newline to stderr. */
const print_error = (line: string): void => console.error(line);

/**
 * `run_publish_napi`'s deps for `checkout`: Node's platform, the file operations above
 * (`is_file` and `mode` following symlinks), `npm` run in the checkout, and the two print
 * streams.
 */
export const create_publish_napi_deps = (
	checkout: string,
	programs: Pick<ReleasePrograms, 'npm'> = RELEASE_PROGRAMS
): PublishNapiDeps => ({
	checkout,
	platform: process.platform,
	read_text: node_release_fs.read_text,
	is_file: node_release_fs.is_file,
	set_mode: node_release_fs.set_mode,
	mode: node_release_fs.mode,
	npm: (args) => node_capture(programs.npm, args, { cwd: checkout }),
	print,
	print_error
});

/** `run_release_notes`' deps for `checkout`: the text read and the two print streams. */
export const create_release_notes_deps = (checkout: string): ReleaseNotesDeps => ({
	checkout,
	read_text: node_release_fs.read_text,
	print,
	print_error
});

/**
 * `run_release_assets`' deps for `checkout`: the working directory, `npm` and `tar` run in it,
 * a real pause between registry attempts, the file operations above (`is_regular_file` not
 * following a symlink), and the two print streams.
 */
export const create_release_assets_deps = (
	checkout: string,
	programs: ReleasePrograms = RELEASE_PROGRAMS
): ReleaseAssetsDeps => ({
	checkout,
	cwd: process.cwd(),
	npm: (args) => node_capture(programs.npm, args),
	tar: (args) => node_capture(programs.tar, args),
	sleep: async (ms) => {
		await sleep(ms);
	},
	fs: {
		make_dir: node_release_fs.make_dir,
		make_temp_dir: node_release_fs.make_temp_dir,
		remove_dir: node_release_fs.remove_dir,
		copy_file: node_release_fs.copy_file,
		is_regular_file: node_release_fs.is_regular_file,
		size: node_release_fs.size,
		read_bytes: node_release_fs.read_bytes,
		write_text: node_release_fs.write_text
	},
	print,
	print_error
});

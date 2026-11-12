/**
 * The two ways the local release driver runs a program over a gro `SpawnProgram` handle: one
 * that captures, for questions whose answer it reads, and one that streams, for long or
 * interactive steps the owner watches. Neither exits; callers decide what a failure means.
 * Which environment a child gets is the handle's, fixed by the root that made it.
 *
 * @module
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram, SpawnResult } from '@fuzdev/gro/spawn.ts';

import { signal_exit_code } from '../exit_code.ts';

/** How a call ended: its outcome and exit code, `128 + N` for a child signal `N` ended, `127`
 * for a program that isn't there. */
export interface Ended {
	success: boolean;
	code: number;
}

/** A finished call's `Ended`. */
export const to_ended = (result: SpawnResult): Ended => {
	if (result.kind === 'exited') return { success: result.code === 0, code: result.code };
	if (result.kind === 'signaled') return { success: false, code: signal_exit_code(result.signal) };
	return {
		success: false,
		code: result.code ?? (result.signal === null ? 1 : signal_exit_code(result.signal))
	};
};

/** A captured call's outcome and output. */
export interface Captured {
	/** Whether it exited 0 with its whole output read. */
	success: boolean;
	/** Its stdout with trailing whitespace trimmed. Leading space is kept, since porcelain
	 * status lines open with it. */
	out: string;
	/** Its stderr, trimmed. */
	err: string;
}

/** Where a call runs, and for a captured one, its stdin. */
export interface ProgramCallOptions {
	cwd: FsReadDir;
	/** `'inherit'` for a call that may prompt (ssh, a credential helper, a pinentry), which
	 * keeps it in the terminal's session; `/dev/null` otherwise. */
	stdin?: 'inherit';
}

/**
 * Run `program` with its output captured. A missing program (`null`) is a failure whose `err`
 * names it, never a throw, and so is output cut short, which no answer may be read from.
 * @param name - the program's name, for the message when it's missing
 */
export const program_capture = async (
	program: SpawnProgram | null,
	name: string,
	args: ReadonlyArray<string>,
	options: ProgramCallOptions
): Promise<Captured> => {
	if (program === null) return { success: false, out: '', err: `${name}: command not found` };
	const result = await program.out(
		args,
		options.stdin === undefined ? { cwd: options.cwd } : { cwd: options.cwd, stdin: options.stdin }
	);
	return {
		success: to_ended(result).success && !result.truncated,
		out: result.stdout.trimEnd(),
		err: result.stderr.trim()
	};
};

/**
 * Run `program` with the terminal attached, stdin included, so a prompt such as npm's browser
 * auth reaches the owner. A missing program (`null`) prints that it's missing and ends `127`.
 * @param name - the program's name, for the message when it's missing
 * @param print_error - where that message goes
 */
export const program_stream = async (
	program: SpawnProgram | null,
	name: string,
	args: ReadonlyArray<string>,
	options: ProgramCallOptions,
	print_error: (line: string) => void
): Promise<Ended> => {
	if (program === null) {
		print_error(`${name}: command not found`);
		return { success: false, code: 127 };
	}
	return to_ended(await program.run(args, { cwd: options.cwd }));
};

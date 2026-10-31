/**
 * Subprocesses for the release scripts: one that captures, for questions, and one that streams,
 * for long or interactive steps. Neither exits the process; callers decide what a failure means.
 *
 * @module
 */

/** The outcome of a release step that can fail: its value's fields on success, a message
 * naming the failure otherwise. */
export type Result<TValue extends object = object> =
	({ ok: true } & TValue) | { ok: false; error: string };

const decoder = new TextDecoder();

export interface CommandOptions {
	cwd?: string;
	env?: Record<string, string>;
}

export interface CommandCapture {
	success: boolean;
	/** The command's stdout with trailing whitespace trimmed. Leading space is kept, since
	 * porcelain status lines open with it. */
	out: string;
	/** The command's stderr, trimmed. */
	err: string;
}

/** Run a command silently and return its outcome and output. A missing binary is a failure
 * whose `err` names it, never a throw. */
export const command_capture = async (
	cmd: string,
	args: ReadonlyArray<string>,
	options: CommandOptions = {}
): Promise<CommandCapture> => {
	try {
		const { success, stdout, stderr } = await new Deno.Command(cmd, {
			args: [...args],
			cwd: options.cwd,
			env: options.env,
			stdout: 'piped',
			stderr: 'piped'
		}).output();
		return { success, out: decoder.decode(stdout).trimEnd(), err: decoder.decode(stderr).trim() };
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;
		return { success: false, out: '', err: `${cmd}: command not found` };
	}
};

export interface CommandStream {
	success: boolean;
	code: number;
}

/** Run a command with the terminal attached (stdin included, so an interactive prompt such as
 * npm's browser auth reaches the user) and return its exit. A missing binary exits 127. */
export const command_stream = async (
	cmd: string,
	args: ReadonlyArray<string>,
	options: CommandOptions = {}
): Promise<CommandStream> => {
	try {
		const { success, code } = await new Deno.Command(cmd, {
			args: [...args],
			cwd: options.cwd,
			env: options.env,
			stdin: 'inherit',
			stdout: 'inherit',
			stderr: 'inherit'
		}).output();
		return { success, code };
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;
		console.error(`${cmd}: command not found`);
		return { success: false, code: 127 };
	}
};

export interface CommandTee extends CommandStream {
	/** The command's stderr, as it was also shown. */
	err: string;
}

/** Run a command with its output shown as it runs, keeping a copy of its stderr for the caller
 * to read the failure from (a registry's refusal, say). No stdin: nothing here prompts. A
 * missing binary exits 127. */
export const command_tee = async (
	cmd: string,
	args: ReadonlyArray<string>,
	options: CommandOptions = {}
): Promise<CommandTee> => {
	let child: Deno.ChildProcess;
	try {
		child = new Deno.Command(cmd, {
			args: [...args],
			cwd: options.cwd,
			env: options.env,
			stdin: 'null',
			stdout: 'inherit',
			stderr: 'piped'
		}).spawn();
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;
		console.error(`${cmd}: command not found`);
		return { success: false, code: 127, err: `${cmd}: command not found` };
	}
	const chunks: Array<string> = [];
	const stream_decoder = new TextDecoder();
	for await (const chunk of child.stderr) {
		for (let written = 0; written < chunk.length;) {
			written += await Deno.stderr.write(chunk.subarray(written));
		}
		chunks.push(stream_decoder.decode(chunk, { stream: true }));
	}
	chunks.push(stream_decoder.decode());
	const { success, code } = await child.status;
	return { success, code, err: chunks.join('') };
};

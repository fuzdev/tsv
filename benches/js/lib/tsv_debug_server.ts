/**
 * One long-lived `tsv_debug <command> --stdin` process, asked one NDJSON request at a time —
 * the client the corpus tools share (`loc_wire_client.ts` for `loc_wires`, `strip_corpus.ts`
 * for `strip`, `imports_corpus.ts` for `imports`). Each command's module doc
 * (`crates/tsv_debug/src/cli/commands/<command>.rs`) states its request and reply; the server
 * loop they share answers every request with exactly one line, flushed before it reads the
 * next (`serve_ndjson`). One process for the whole run, since a spawn per file would cost the
 * run its process startup over and over and report a panic as an exit code rather than as a
 * panic.
 *
 * The binary is the shared `--profile corpus` build world (`npm run build:check`), where a
 * panic unwinds and is answered as one, guarded for freshness like the N-API addon. The
 * process comes from the caller, which makes the program once the guard passes
 * (`TsvDebugProgram`, which a gro `SpawnProgram` satisfies), so the module imports no
 * package, not even for a type.
 *
 * @module
 */

import { signal_exit_code } from '../../../scripts/exit_code.ts';
import { check_artifact_freshness } from './check_artifact_freshness.ts';
import { lines_of } from './text_lines.ts';

/** The `--profile corpus` `tsv_debug` binary `npm run build:check` builds. */
export const TSV_DEBUG_CORPUS = 'target/corpus/tsv_debug';

/**
 * What `tsv_debug` reads from the environment beyond a spawn's base: the panic hook's
 * backtrace switches, so a panic's report on stderr reads as the caller asked. The commands
 * served here start no sidecar.
 */
export const TSV_DEBUG_PASS_ENV: ReadonlyArray<string> = ['RUST_BACKTRACE', 'RUST_LIB_BACKTRACE'];

/** How a started process ended — a gro `SpawnResult` is one. */
export type TsvDebugEnd =
	| { readonly kind: 'exited'; readonly code: number }
	| { readonly kind: 'signaled'; readonly signal: string }
	| {
			readonly kind: 'timed_out' | 'aborted';
			readonly code: number | null;
			readonly signal: string | null;
	  };

/** A started `tsv_debug <command> --stdin` — a gro `SpawnChild` is one. */
export interface TsvDebugChild {
	readonly stdin: {
		/** Resolves `false` once the process closed its stdin or ended. */
		write(data: string): Promise<boolean>;
		close(): Promise<void>;
	} | null;
	readonly stdout: AsyncIterable<Uint8Array> | null;
	readonly closed: Promise<TsvDebugEnd>;
	/** Signals the process; `false` once it has ended. */
	kill(): boolean;
}

/** The `tsv_debug` binary — a gro `SpawnProgram` is one. */
export interface TsvDebugProgram {
	start(
		args: ReadonlyArray<string>,
		options: { stdio: readonly ['pipe', 'pipe', 'inherit'] }
	): Promise<TsvDebugChild>;
}

/** A running `tsv_debug <command> --stdin`, one request in flight at a time. */
export class TsvDebugServer<Request, Reply> {
	private readonly command: string;
	private readonly child: TsvDebugChild;
	private readonly stdin: NonNullable<TsvDebugChild['stdin']>;
	private readonly replies: AsyncGenerator<string>;

	private constructor(command: string, child: TsvDebugChild) {
		if (child.stdin === null || child.stdout === null) {
			throw new Error(`${TSV_DEBUG_CORPUS} ${command} --stdin started without piped stdio`);
		}
		this.command = command;
		this.child = child;
		this.stdin = child.stdin;
		this.replies = lines_of(child.stdout);
	}

	/**
	 * Spawn `tsv_debug <command> --stdin`, refusing a missing or stale binary
	 * (`BENCH_STALE_OK=1` overrides staleness). Its stderr is the caller's.
	 * @param open_program - makes the program, called once the binary passed the guard
	 */
	static async start<Request, Reply>(
		command: string,
		open_program: () => TsvDebugProgram
	): Promise<TsvDebugServer<Request, Reply>> {
		await check_artifact_freshness([
			{
				label: 'tsv_debug (corpus)',
				path: TSV_DEBUG_CORPUS,
				binding_crates: ['tsv_debug', 'tsv_cli'],
				rebuild: 'npm run build:check'
			}
		]);
		return TsvDebugServer.spawn(command, open_program());
	}

	/** Spawn `<program> <command> --stdin` with no freshness guard: `start`'s, once it passed. */
	static async spawn<Request, Reply>(
		command: string,
		program: TsvDebugProgram
	): Promise<TsvDebugServer<Request, Reply>> {
		const child = await program.start([command, '--stdin'], { stdio: ['pipe', 'pipe', 'inherit'] });
		return new TsvDebugServer(command, child);
	}

	/**
	 * Send `request` and read its reply line, parsed — or throw when the process exits
	 * without answering (a malformed request ends it, its reason on stderr).
	 *
	 * Not safe for concurrent calls: requests are not serialized, so await each before the next.
	 */
	async ask(request: Request): Promise<Reply> {
		// `false` when the process already closed its stdin or ended: no answer can come
		if (!(await this.stdin.write(JSON.stringify(request) + '\n'))) throw this.unanswered();
		const { value: line, done } = await this.replies.next();
		if (done || line === undefined) throw this.unanswered();
		return JSON.parse(line) as Reply;
	}

	private unanswered(): Error {
		return new Error(
			`${TSV_DEBUG_CORPUS} ${this.command} --stdin exited without answering — see its stderr above`
		);
	}

	/**
	 * Close stdin and wait for the process to exit — a non-zero exit after every reply arrived
	 * is still a failure, thrown, never a quiet end of run. A process a signal `N` ended
	 * exits `128 + N`.
	 */
	async close(): Promise<void> {
		await this.stdin.close();
		const code = to_end_code(await this.child.closed);
		if (code !== 0) {
			throw new Error(`${TSV_DEBUG_CORPUS} ${this.command} --stdin exited ${code} on close`);
		}
	}

	/** Stop the process without waiting for it, for a run that ends early. */
	kill(): void {
		this.child.kill();
	}
}

/** The code a process ended with: its exit code, `128 + N` for signal `N`. */
const to_end_code = (end: TsvDebugEnd): number => {
	if (end.kind === 'exited') return end.code;
	if (end.kind === 'signaled') return signal_exit_code(end.signal);
	return end.signal === null ? (end.code ?? 1) : signal_exit_code(end.signal);
};

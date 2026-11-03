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
 * Deno-only, like every corpus tool. The binary is the shared `--profile corpus` build world
 * (`deno task build:check`), where a panic unwinds and is answered as one, guarded for
 * freshness like the FFI library. Node-modules-free.
 *
 * @module
 */

import { check_artifact_freshness } from './check_artifact_freshness.ts';
import { lines_of } from './text_lines.ts';

/** The `--profile corpus` `tsv_debug` binary `deno task build:check` builds. */
export const TSV_DEBUG_CORPUS = 'target/corpus/tsv_debug';

/** A running `tsv_debug <command> --stdin`, one request in flight at a time. */
export class TsvDebugServer<Request, Reply> {
	private readonly command: string;
	private readonly child: Deno.ChildProcess;
	private readonly writer: WritableStreamDefaultWriter<Uint8Array>;
	private readonly replies: AsyncGenerator<string>;
	private readonly encoder = new TextEncoder();

	private constructor(command: string, child: Deno.ChildProcess) {
		this.command = command;
		this.child = child;
		this.writer = child.stdin.getWriter();
		this.replies = lines_of(child.stdout);
	}

	/**
	 * Spawn `tsv_debug <command> --stdin`, refusing a missing or stale binary
	 * (`BENCH_STALE_OK=1` overrides staleness). Its stderr is the caller's.
	 */
	static async start<Request, Reply>(command: string): Promise<TsvDebugServer<Request, Reply>> {
		await check_artifact_freshness([
			{
				label: 'tsv_debug (corpus)',
				path: TSV_DEBUG_CORPUS,
				binding_crates: ['tsv_debug', 'tsv_cli'],
				rebuild: 'deno task build:check'
			}
		]);
		const child = new Deno.Command(TSV_DEBUG_CORPUS, {
			args: [command, '--stdin'],
			stdin: 'piped',
			stdout: 'piped',
			stderr: 'inherit'
		}).spawn();
		return new TsvDebugServer(command, child);
	}

	/**
	 * Send `request` and read its reply line, parsed — or throw when the process exits
	 * without answering (a malformed request ends it, its reason on stderr).
	 *
	 * Not safe for concurrent calls: requests are not serialized, so await each before the next.
	 */
	async ask(request: Request): Promise<Reply> {
		await this.writer.write(this.encoder.encode(JSON.stringify(request) + '\n'));
		const { value: line, done } = await this.replies.next();
		if (done || line === undefined) {
			throw new Error(
				`${TSV_DEBUG_CORPUS} ${this.command} --stdin exited without answering — see its stderr above`
			);
		}
		return JSON.parse(line) as Reply;
	}

	/**
	 * Close stdin and wait for the process to exit — a non-zero exit after every reply arrived
	 * is still a failure, thrown, never a quiet end of run.
	 */
	async close(): Promise<void> {
		await this.writer.close();
		const status = await this.child.status;
		if (!status.success) {
			throw new Error(`${TSV_DEBUG_CORPUS} ${this.command} --stdin exited ${status.code} on close`);
		}
	}
}

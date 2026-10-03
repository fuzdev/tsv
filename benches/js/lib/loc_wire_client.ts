/**
 * The loc-bearing parse wire for the corpus tools, from one long-lived `tsv_debug` process.
 *
 * Every binding emits the span-only wire — `start`/`end` offsets, no per-node `loc` — so the
 * Rust `loc` emitter is reachable through no binding: only `tsv parse --locations`, the
 * fixtures, and `tsv_debug`. `corpus_compare_parse.ts`'s loc arm grades that emitter (against
 * the definition, then against the canonical parser's own `loc`), so it asks
 * `tsv_debug loc_wires --stdin` for it: one NDJSON request per document, one reply line each
 * (the command's module doc in `crates/tsv_debug/src/cli/commands/loc_wires.rs`). One process
 * for the whole run — a per-file `tsv parse --locations` spawn would cost the conformance
 * cadence a minute of process startup, push every source through argv or a pipe per file, and
 * report a panic as an exit code rather than as a panic.
 *
 * The verdict semantics are the FFI's, so the two arms read one verdict: a parse error
 * throws its message exactly as `tsv_ffi` renders it, and a caught
 * panic throws `panic: <payload>` — the shape `is_native_panic_error` recognizes, so a panic
 * on this wire still fails `gate_on_panics` rather than counting as one more rejection. The
 * `corpus` profile is what makes a panic catchable here, as it is for the FFI library.
 *
 * Deno-only, like every corpus tool. The binary is the shared `--profile corpus` build world
 * (`deno task build:check`), guarded for freshness like the FFI library.
 */

import { check_artifact_freshness } from './check_artifact_freshness.ts';
import { lines_of } from './text_lines.ts';
import { goal_for, type Language, type ParseGoal } from './types.ts';

/** The `--profile corpus` `tsv_debug` binary `deno task build:check` builds. */
export const TSV_DEBUG_CORPUS = 'target/corpus/tsv_debug';

/** One reply line (the command's module doc). */
type WireReply = { loc: unknown } | { error: string } | { panic: string };

/** A running `tsv_debug loc_wires --stdin`, asked one document at a time. */
export class LocWireClient {
	private readonly child: Deno.ChildProcess;
	private readonly writer: WritableStreamDefaultWriter<Uint8Array>;
	private readonly replies: AsyncGenerator<string>;
	private readonly encoder = new TextEncoder();

	private constructor(child: Deno.ChildProcess) {
		this.child = child;
		this.writer = child.stdin.getWriter();
		this.replies = lines_of(child.stdout);
	}

	/** Spawn the server, refusing a missing or stale binary (`BENCH_STALE_OK=1` overrides staleness). */
	static async start(): Promise<LocWireClient> {
		await check_artifact_freshness([
			{
				label: 'tsv_debug (corpus)',
				path: TSV_DEBUG_CORPUS,
				binding_crates: ['tsv_debug', 'tsv_cli'],
				rebuild: 'deno task build:check'
			}
		]);
		const child = new Deno.Command(TSV_DEBUG_CORPUS, {
			args: ['loc_wires', '--stdin'],
			stdin: 'piped',
			stdout: 'piped',
			stderr: 'inherit'
		}).spawn();
		return new LocWireClient(child);
	}

	/**
	 * The loc wire of `source`, parsed — or a throw carrying tsv's rejection, or
	 * `panic: <payload>` when tsv panicked. `goal` reaches TypeScript only (`goal_for`), as
	 * it reaches the FFI.
	 *
	 * The source is sent well-formed: a lone surrogate has no UTF-8 spelling, and the FFI
	 * path (`TextEncoder.encodeInto`) replaces one with U+FFFD — so does this, and the two
	 * wires parse the same bytes.
	 *
	 * Not safe for concurrent calls: requests are not serialized, so await each before the next.
	 */
	async parse(source: string, language: Language, goal?: ParseGoal): Promise<unknown> {
		const request_goal = goal_for(language, goal);
		const request = JSON.stringify({
			language,
			...(request_goal === undefined ? {} : { goal: request_goal }),
			source: source.toWellFormed()
		});
		await this.writer.write(this.encoder.encode(request + '\n'));
		const { value: line, done } = await this.replies.next();
		if (done || line === undefined) {
			throw new Error(
				`${TSV_DEBUG_CORPUS} loc_wires --stdin exited without answering — see its stderr above`
			);
		}
		const reply = JSON.parse(line) as WireReply;
		if ('loc' in reply) return reply.loc;
		if ('panic' in reply) throw new Error(`panic: ${reply.panic}`);
		throw new Error(reply.error);
	}

	/**
	 * Close stdin and wait for the server to exit — a non-zero exit after every reply
	 * arrived is still a failure, never a quiet end of run.
	 */
	async close(): Promise<void> {
		await this.writer.close();
		const status = await this.child.status;
		if (!status.success) {
			throw new Error(`${TSV_DEBUG_CORPUS} loc_wires --stdin exited ${status.code} on close`);
		}
	}
}

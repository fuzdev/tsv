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
 * The process is the shared `tsv_debug` client's (`tsv_debug_server.ts`): one long-lived
 * `--profile corpus` server, guarded for freshness, started from the program the caller
 * makes.
 */

import { goal_for, type Language, type ParseGoal } from './types.ts';
import { type TsvDebugProgram, TsvDebugServer } from './tsv_debug_server.ts';

/** One request line (the command's module doc). */
type WireRequest = { language: Language; goal?: ParseGoal; source: string };

/** One reply line (the command's module doc). */
type WireReply = { loc: unknown } | { error: string } | { panic: string };

/** A running `tsv_debug loc_wires --stdin`, asked one document at a time. */
export class LocWireClient {
	private readonly server: TsvDebugServer<WireRequest, WireReply>;

	private constructor(server: TsvDebugServer<WireRequest, WireReply>) {
		this.server = server;
	}

	/**
	 * Spawn the server, refusing a missing or stale binary (`BENCH_STALE_OK=1` overrides staleness).
	 * @param open_program - makes the `tsv_debug` program, called once the binary passed the guard
	 */
	static async start(open_program: () => TsvDebugProgram): Promise<LocWireClient> {
		return new LocWireClient(
			await TsvDebugServer.start<WireRequest, WireReply>('loc_wires', open_program)
		);
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
		const reply = await this.server.ask({
			language,
			...(request_goal === undefined ? {} : { goal: request_goal }),
			source: source.toWellFormed()
		});
		if ('loc' in reply) return reply.loc;
		if ('panic' in reply) throw new Error(`panic: ${reply.panic}`);
		throw new Error(reply.error);
	}

	/**
	 * Close stdin and wait for the server to exit — a non-zero exit after every reply
	 * arrived is still a failure, never a quiet end of run.
	 */
	async close(): Promise<void> {
		await this.server.close();
	}
}

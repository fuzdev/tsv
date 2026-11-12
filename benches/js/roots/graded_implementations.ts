/**
 * What the roots of the tools that grade every implementation share, not a root itself: the
 * harness's whole set (`init_implementations`), tsv's N-API addon at `COMPARE_PROFILE` and its
 * `nodejs` WASM bundle among them, behind the executed-artifact freshness guard. The set's init
 * runs one program: `rsvelte-fmt`'s platform binary from the bench install (`lib/rsvelte.ts`),
 * through `node:child_process` with the inherited environment plus the `PRETTIER_DEBUG=1` the
 * harness sets, for its version and its config, embed, and reject probes, though no tool that
 * loads the set reads its row. Its closure
 * reaches the Deno row's C-FFI binding beside the N-API one, so `typecheck:js`'s `deno check`
 * checks it and the roots that import it, as it checks the bench's own entries, and
 * `tsconfig.bench.json` leaves them out.
 *
 * @module
 */

import { type InitializedImplementations, init_implementations } from '../lib/implementations.ts';
import { COMPARE_PROFILE } from '../lib/tsv_artifacts.ts';
import type { Logger } from '../lib/types.ts';

/** Every implementation, tsv's at `COMPARE_PROFILE`, logging each init to `logger`. */
export const init_graded_implementations = (logger: Logger): Promise<InitializedImplementations> =>
	init_implementations({ logger, native_profile: COMPARE_PROFILE });

/**
 * `benches/js/diagnostics/wasm_json_probe.ts`'s root. Its reach: the `gates` corpus view, read
 * through the corpus loader, which Bun's bench runs too and so reads with `node:fs`, a named
 * exception whose reason is Bun; and every implementation the harness knows, tsv's
 * profile-guided N-API addon and its `nodejs` WASM bundle among them. It writes nothing. The one
 * program it runs is the set's: `rsvelte-fmt`'s platform binary from the bench install
 * (`lib/rsvelte.ts`), through `node:child_process` with the inherited environment plus the
 * `PRETTIER_DEBUG=1` the harness sets, for its version and its config, embed, and reject probes
 * at init, a row this tool doesn't read. Run it from the checkout's root, where the corpus
 * paths resolve.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { run_wasm_json_probe } from '../diagnostics/wasm_json_probe.ts';
import { CorpusLoader } from '../lib/corpus.ts';
import { init_implementations } from '../lib/implementations.ts';

await run_wasm_json_probe({
	load_files: (logger) => new CorpusLoader('gates').load(logger),
	init_implementations: (logger) => init_implementations({ logger })
});
process.exit(0);

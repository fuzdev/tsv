/**
 * `corpus:stats`' root (`benches/js/diagnostics/corpus_stats.ts`). Its reach is the library's:
 * the corpus, read through the corpus loader, which Bun's bench runs too and so reads with
 * `node:fs`, a named exception whose reason is Bun. It writes nothing and runs nothing but the
 * loader's own `git`. Run it from the checkout's root, where the loader's paths resolve.
 *
 * @module
 */

import { set_exit_code } from '@fuzdev/gro/exit.ts';
import { args } from '@fuzdev/gro/args.ts';

import { run_corpus_stats } from '../diagnostics/corpus_stats.ts';

set_exit_code(await run_corpus_stats(args));

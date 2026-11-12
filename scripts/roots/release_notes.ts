/**
 * `release:notes`' entry: a version's `CHANGELOG.md` section printed, the GitHub Release body
 * (`scripts/release_notes.ts`).
 *
 * A named exception to the host-module rule: this entry and everything it imports use Node's
 * own `node:*` modules and no package, because the release workflow's `release` job runs it
 * holding the repository's `contents: write` and `id-token: write`, and so installs nothing
 * (docs/tasks.md §`release:notes`). The root config's Deno walk (`typecheck:scripts`) checks
 * this module with no `node_modules` in view, so a package import here or beneath it fails
 * `check`.
 *
 * The checkout is this module's own location, never the working directory. It reads
 * `CHANGELOG.md`, writes nothing, and runs nothing; the notes are its only stdout. Its deps are
 * `create_release_notes_deps`' (`scripts/release/node_host.ts`).
 *
 * @module
 */

import process from 'node:process';

import { create_release_notes_deps, entry_checkout } from '../release/node_host.ts';
import { run_release_notes } from '../release_notes.ts';

process.exitCode = await run_release_notes(
	create_release_notes_deps(entry_checkout(import.meta.url)),
	process.argv.slice(2)
);

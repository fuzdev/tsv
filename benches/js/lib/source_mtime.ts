/**
 * The newest source mtime behind a built artifact, read through a `SourceTree`: the one walk
 * every mtime guard dates an artifact by, whatever reaches the files. The run-side guard
 * (`check_artifact_freshness.ts`) and the staged-package guard (`scripts/check_staged_freshness.ts`)
 * read the checkout through Node's `node:fs`, under every runtime the bench runs on; the
 * build-side skip (`scripts/run_if_stale.ts`) reads it through a directory handle. Two
 * bindings, one walk, so the build side and the run side agree about what feeds a crate,
 * but for a symlink under a crate's `src/`, which each binding reads its own way.
 *
 * Imports nothing, so every one of those readers can load it.
 *
 * @module
 */

/** The newest mtime a walk found, and where. */
export interface SourceMtime {
	/** Newest mtime in milliseconds (0 if no sources were found). */
	ms: number;
	/** The newest source's path, for the message: `crates/`-relative for a crate's file, the
	 * file's name for a workspace file. */
	path: string;
}

/** What a walk reads, every path relative to the checkout and `/`-separated. */
export interface SourceTree {
	/**
	 * The entry at `path`, a final symlink followed.
	 * @returns the entry's mtime and whether it's a file, or `null` when nothing is there
	 * @throws when what is there can't be read
	 */
	stat: (path: string) => Promise<{ mtime_ms: number; is_file: boolean } | null>;
	/**
	 * Every entry beneath the directory `path`, recursively, relative to it. The handle binding
	 * lists a symlink to a directory and doesn't descend; `node:fs`'s recursive listing (Node,
	 * Bun) descends into it.
	 * @returns the entries, or none when nothing is at `path`
	 * @throws when what is there can't be listed
	 */
	list: (path: string) => Promise<Array<string>>;
}

/**
 * `op` on `path`, its failure named by the path: a walk that can't read a source can't date
 * what it builds, so it fails rather than answering as though the source weren't there.
 */
const read_source = async <T>(path: string, op: () => Promise<T>): Promise<T> => {
	try {
		return await op();
	} catch (error) {
		const reason = error instanceof Error ? error.message : String(error);
		throw new Error(`can't read ${path} to date what it builds: ${reason}`, { cause: error });
	}
};

/**
 * The workspace-level files every cargo build reads beside its crates' own sources:
 * the root manifest (every `[profile.*]`, the workspace dependencies, the version) and
 * the lockfile (what those dependencies resolved to). Stated once, for every mtime
 * guard — a profile edit that staled one guard's artifact and not another's is two
 * guards disagreeing about what a build is made of.
 */
export const WORKSPACE_BUILD_FILES = ['Cargo.toml', 'Cargo.lock'];

/**
 * Newest mtime across every file under `src/`, the crate's `build.rs`, and its `Cargo.toml`,
 * for the given crates under `crates/`. Every file under `src/`, not `*.rs` alone: a build
 * script's or an `include_*!`'s inputs can sit there too, and a walk that saw only Rust let a
 * codegen change stale nothing on any artifact. A path with nothing at it counts as absent: a
 * crate with no `build.rs` or no `src/`, or an entry gone between the listing and its `stat`.
 * @throws when a source is there but can't be read, the path named
 */
export async function newest_crate_source_mtime(
	tree: SourceTree,
	crates: readonly string[]
): Promise<SourceMtime> {
	let newest: SourceMtime = { ms: 0, path: '' };
	const consider = async (path: string, rel: string, file_only: boolean): Promise<void> => {
		const st = await read_source(path, () => tree.stat(path));
		if (st === null || (file_only && !st.is_file)) return;
		if (st.mtime_ms > newest.ms) newest = { ms: st.mtime_ms, path: rel };
	};

	for (const crate of crates) {
		for (const manifest of ['Cargo.toml', 'build.rs']) {
			await consider(`crates/${crate}/${manifest}`, `${crate}/${manifest}`, false);
		}
		const src = `crates/${crate}/src`;
		for (const relative of await read_source(src, () => tree.list(src))) {
			await consider(`${src}/${relative}`, `${crate}/src/${relative}`, true);
		}
	}
	return newest;
}

/**
 * Newest mtime across `WORKSPACE_BUILD_FILES`, labeled by file name (`ms: 0` when
 * none exists). The floor every cargo-built artifact is dated against, whatever
 * crates feed it. A missing file counts as absent (a fresh clone has no lockfile before
 * its first build), and the crate sources govern.
 * @throws when a file is there but can't be read, the path named
 */
export async function newest_workspace_build_mtime(tree: SourceTree): Promise<SourceMtime> {
	let newest: SourceMtime = { ms: 0, path: '' };
	for (const file of WORKSPACE_BUILD_FILES) {
		const st = await read_source(file, () => tree.stat(file));
		if (st !== null && st.mtime_ms > newest.ms) newest = { ms: st.mtime_ms, path: file };
	}
	return newest;
}

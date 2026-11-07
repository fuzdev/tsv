/**
 * The bench's profile-guided native artifacts: where they live, and the stamp that says
 * what each one is. Every native tsv row measures a profile-guided build — the C-FFI
 * library under Deno, the N-API addon under Node and Bun — so the bench measures what
 * ships (docs/benchmarks.md §PGO).
 *
 * `scripts/pgo/bench.ts` (`deno task build:bench:pgo`) runs the PGO pipeline with the
 * FFI world added (`scripts/pgo/pgo.ts all --ffi`), and only once every gate has passed
 * copies the two libraries here and writes the stamp beside them: each library's sha256,
 * its profile's, and the training it came from. The copy keeps a later pipeline run
 * (which deletes its own build dirs first) from pulling an artifact out from under a run.
 *
 * Every reader keys on the stamp: the freshness guard refuses a library whose bytes the
 * stamp does not name — a plain build copied into the slot, or one a failed pipeline left
 * — and the report discloses the training it records (`native_build`).
 *
 * Node builtins only, like `tsv_artifacts.ts`, which builds its paths from this module.
 *
 * @module
 */

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { relative } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import { native_library_filename } from './runtime.ts';

/** A native binding the bench loads profile-guided. */
export type PgoBenchWorld = 'ffi' | 'napi';

export const PGO_BENCH_WORLDS: readonly PgoBenchWorld[] = ['ffi', 'napi'];

const PROJECT_ROOT = fileURLToPath(new URL('../../..', import.meta.url)).replace(/\/$/, '');

/**
 * Where the gated copies live — under the PGO pipeline's own working directory (`.pgo/`,
 * gitignored), outside `target/`, which no cargo command writes into here.
 */
export const PGO_BENCH_DIR = `${PROJECT_ROOT}/.pgo/bench`;

export const PGO_BENCH_STAMP_PATH = `${PGO_BENCH_DIR}/stamp.json`;

/** The stamp's shape version — bumped when a field changes, so a reader can refuse an older one. */
export const PGO_BENCH_STAMP_VERSION = 1;

/** The profile-guided library the bench loads for `world`. */
export function pgo_bench_library_path(world: PgoBenchWorld): string {
	return `${PGO_BENCH_DIR}/${native_library_filename(world === 'ffi' ? 'tsv_ffi' : 'tsv_napi')}`;
}

/** One gated library, as the stamp records it. */
export interface PgoBenchArtifact {
	/** sha256 of the library's bytes. */
	sha256: string;
	/** sha256 of the merged profile it was built with. */
	profile_sha256: string;
	/** What the profile was trained through — the binary itself, and how it was driven. */
	trained_through: string;
}

/** What `scripts/pgo/bench.ts` writes once the pipeline's gates pass. */
export interface PgoBenchStamp {
	version: number;
	/** When the libraries were staged. */
	generated: string;
	/** The training corpus: the `../corpora` snapshot's collections of one manifest origin. */
	training: {
		origin: 'own';
		collections: string[];
		/** The snapshot's `collections/` tree id the training read, as `GATE_CHECKOUT_IDS` pins it. */
		snapshot_tree: string;
	};
	artifacts: Record<PgoBenchWorld, PgoBenchArtifact>;
}

/** The sha256 of a file's bytes. */
export function sha256_file(path: string): string {
	return createHash('sha256').update(readFileSync(path)).digest('hex');
}

/** The stamp, or `null` when it is absent, unreadable, or of another version. */
export function read_pgo_bench_stamp(): PgoBenchStamp | null {
	try {
		const stamp = JSON.parse(readFileSync(PGO_BENCH_STAMP_PATH, 'utf8')) as PgoBenchStamp;
		return stamp.version === PGO_BENCH_STAMP_VERSION ? stamp : null;
	} catch {
		return null;
	}
}

/**
 * Why the library at `path` is not the gated profile-guided build of `world`, or `null`
 * when the stamp names its bytes. A missing library is the caller's to report.
 */
export function pgo_bench_mismatch(world: PgoBenchWorld, path: string): string | null {
	const stamp = read_pgo_bench_stamp();
	if (stamp === null) return `no PGO stamp at ${PGO_BENCH_STAMP_PATH}`;
	let sha256: string;
	try {
		sha256 = sha256_file(path);
	} catch {
		return null;
	}
	return sha256 === stamp.artifacts[world].sha256
		? null
		: `its bytes are not the gated PGO build the stamp names (${PGO_BENCH_STAMP_PATH})`;
}

/**
 * Refuse a set `TSV_FFI_PROFILE` in a process that measures or smoke-tests the native rows.
 * The variable points the FFI loader at a cargo profile's PLAIN build under `target/` — the
 * corpus tools' knob — and a bench row timing that would be published beside a stamp that
 * names a different library. The corpus and over-acceptance tools keep it; they publish no
 * native timing.
 *
 * @param entry - the refusing entry point, named in the error
 * @throws when `TSV_FFI_PROFILE` is set
 */
export function refuse_ffi_profile_override(entry: string): void {
	const profile = process.env.TSV_FFI_PROFILE;
	if (profile === undefined) return;
	throw new Error(
		`${entry}: TSV_FFI_PROFILE=${profile} is set, which loads target/${profile}'s plain FFI library — ` +
			'the bench and smoke measure the profile-guided one (`deno task build:bench:pgo`); unset it'
	);
}

/**
 * The report's disclosure of the native rows' build (`native_build`, report `version` 23):
 * the library this runtime's native rows actually loaded — the FFI library under Deno, the
 * N-API addon under Node and Bun, the `+locations` and `-internal` variants included — hashed,
 * and called profile-guided only when the stamp names those exact bytes.
 */
export interface NativeBuild {
	/**
	 * `pgo` when the stamp names the loaded library's bytes, `plain` when a stamp exists and
	 * names other bytes, `null` when there is no stamp to say.
	 */
	build: 'pgo' | 'plain' | null;
	/**
	 * The library the native rows loaded: its binding, its path relative to the tsv checkout
	 * (never absolute — the report is published, and a home directory is no part of it) and
	 * its sha256.
	 */
	loaded: { binding: PgoBenchWorld; path: string; sha256: string };
	/** What the stamp names — each binding's gated library and its profile's training — or
	 * `null` with no stamp. */
	bindings: Record<PgoBenchWorld, PgoBenchArtifact> | null;
	/** The training corpus (`PgoBenchStamp.training`), or `null` with no stamp. */
	training: PgoBenchStamp['training'] | null;
	/**
	 * Whether no collection the training read is one the run's corpus loaded — the held-out
	 * guarantee, read off the run's own corpus sources by collection name. `null` when it
	 * cannot be computed: no stamp, or no snapshot collection in the corpus (the coverage-only
	 * conformance surface, which times nothing).
	 */
	disjoint_from_timed_corpus: boolean | null;
	/** When `build:bench:pgo` staged the libraries, or `null` with no stamp. */
	staged: string | null;
}

/**
 * The disclosure for a run whose native rows loaded `loaded` (`binding` + `path`), over a
 * corpus holding the snapshot collections `corpus_collections` (`null` when it holds none).
 */
export function native_build_disclosure(
	loaded: { binding: PgoBenchWorld; path: string },
	corpus_collections: readonly string[] | null
): NativeBuild {
	const stamp = read_pgo_bench_stamp();
	const sha256 = sha256_file(loaded.path);
	const disjoint =
		stamp === null || corpus_collections === null || corpus_collections.length === 0
			? null
			: !stamp.training.collections.some((name) => corpus_collections.includes(name));
	return {
		build:
			stamp === null ? null : stamp.artifacts[loaded.binding].sha256 === sha256 ? 'pgo' : 'plain',
		loaded: { ...loaded, path: relative(PROJECT_ROOT, loaded.path), sha256 },
		bindings: stamp?.artifacts ?? null,
		training: stamp?.training ?? null,
		disjoint_from_timed_corpus: disjoint,
		staged: stamp?.generated ?? null
	};
}

/** The disclosure's one markdown line, without its label. */
export function native_build_summary(native: NativeBuild): string {
	const binding = native.loaded.binding === 'ffi' ? 'the FFI library' : 'the N-API addon';
	if (native.build !== 'pgo' || native.training === null || native.bindings === null) {
		return (
			`⚠ the native tsv rows loaded ${binding} at \`${native.loaded.path}\`, whose bytes ` +
			(native.build === null
				? 'no PGO stamp describes — its build is unknown'
				: 'the PGO stamp does not name — a plain build, not what ships') +
			'. WASM rows are not profile-guided.'
		);
	}
	const disjoint =
		native.disjoint_from_timed_corpus === null
			? 'whose overlap with this run’s corpus is not computable here (no snapshot collection in it)'
			: native.disjoint_from_timed_corpus
				? 'none of which this run’s corpus holds'
				: '⚠ some of which this run’s corpus ALSO holds, so these numbers are not held out';
	const trained =
		native.loaded.binding === 'ffi'
			? `The FFI library's profile is its own: ${native.bindings.ffi.trained_through}.`
			: `The N-API addon's is the shipped addon's: ${native.bindings.napi.trained_through}.`;
	return (
		`the native tsv rows (${native.loaded.binding === 'ffi' ? 'FFI, under Deno' : 'N-API, under Node/Bun'}) ` +
		`measure ${binding}'s profile-guided build, trained on the corpus snapshot's ` +
		`\`origin: ${native.training.origin}\` collections — ${disjoint} ` +
		`(${native.training.collections.join(', ')}). ${trained} WASM rows are not profile-guided.`
	);
}

/**
 * The composed report's one line, for siblings that all loaded one stamp's profile-guided
 * pair: both bindings, each with its profile's training.
 */
export function native_build_pair_summary(native: NativeBuild, disjoint: boolean | null): string {
	if (native.build !== 'pgo' || native.training === null || native.bindings === null) {
		return native_build_summary(native);
	}
	const held_out =
		disjoint === null
			? 'whose overlap with the siblings’ corpora is not computable here'
			: disjoint
				? 'none of which any sibling’s corpus holds'
				: '⚠ some of which a sibling’s corpus ALSO holds, so those numbers are not held out';
	return (
		'every native tsv row (FFI under Deno, N-API under Node/Bun) measures a profile-guided ' +
		`build, trained on the corpus snapshot's \`origin: ${native.training.origin}\` collections — ` +
		`${held_out} (${native.training.collections.join(', ')}). The FFI library's profile is its own: ` +
		`${native.bindings.ffi.trained_through}. The N-API addon's is the shipped addon's: ` +
		`${native.bindings.napi.trained_through}. WASM rows are not profile-guided.`
	);
}

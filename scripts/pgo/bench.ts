/**
 * The bench's profile-guided native pair — `npm run build:bench:pgo`, which every bench
 * build runs (`build:bench`). The bench measures what ships: every native tsv row loads a
 * profile-guided build (docs/benchmarks.md §PGO), the N-API addon from the shipped addon's
 * own pipeline and the C-FFI library from the same pipeline's third world.
 *
 * It runs that pipeline (`scripts/pgo/pgo.ts all --ffi --reuse`, its working directory
 * `.pgo/`, shared with `build:pgo`), which skips the build and its gates when its stamp says
 * the binaries there were built and gated from exactly this run's inputs — so a bench build
 * reuses what `build:pgo` produced and the other way round. The depth gate is the shipped
 * addon's release check, not a measurement one, so it is skipped here; the gates that guard
 * what the bench measures (a non-empty profile applied to every crate, no instrumentation in
 * the binaries, output byte-identical to the plain builds) are the pipeline's, passed for these
 * bytes or the pipeline fails. `TSV_BUILD_FORCE=1` drops `--reuse`, rebuilding regardless —
 * what a change the key cannot see needs.
 *
 * The pipeline runs as a child with the terminal and the whole environment: its stamp key
 * hashes the cargo and rustc variables (`CARGO_*`, `RUSTC*`, `RUSTFLAGS`, `RUSTDOC*`), so a
 * narrower environment would give this run another key than `build:pgo` and the two would stop
 * sharing `.pgo/`. Its Node is the `node` on `PATH`, which can be another Node than the one
 * running this; the key leaves the Node version out, so that changes no key.
 *
 * Then it copies the two libraries into the bench's slot and writes the bench stamp — the
 * stamp removed first and written last, so an interrupted copy leaves none that vouches for
 * it. The copy is made on every run, so a library's mtime says when its build was last
 * confirmed current, the reading the bench's mtime guard needs, and it keeps the built file's
 * mode. A side effect worth knowing: the pipeline stages its PGO pair into
 * `crates/tsv_napi/pkg/<triple>/`, as `build:pgo` does.
 *
 * Exits 1 when the pipeline fails (staging nothing) or when a library or profile on disk is
 * not what the pipeline's stamp records its gates passing; a pipeline a signal `N` ended reads
 * as exit `128 + N` in the message.
 *
 * Reach: the checkout, the child's working directory, with `.pgo/` beneath it read and its
 * `bench/` written; and the `node` the root grants (`PgoBenchDeps`, built by
 * `scripts/roots/pgo_bench.ts`).
 *
 * @module
 */

import type { FsDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram, SpawnResult } from '@fuzdev/gro/spawn.ts';

import { CORPORA_ROOT } from '../../benches/js/lib/corpora.ts';
import { GATE_CHECKOUT_IDS } from '../../benches/js/lib/gate_counts.ts';
import {
	PGO_BENCH_DIR,
	PGO_BENCH_STAMP_NAME,
	PGO_BENCH_STAMP_VERSION,
	PGO_BENCH_SUBDIR,
	PGO_BENCH_WORLDS,
	type PgoBenchStamp,
	type PgoBenchWorld,
	pgo_bench_library_name
} from '../../benches/js/lib/pgo_bench.ts';
import { signal_exit_code } from '../exit_code.ts';
import { cargo_artifacts, pgo_paths } from './layout.ts';

/** The pipeline's working directory — `pgo.ts`'s default, so `build:pgo` shares it. */
const PGO_DIR = '.pgo';

/** What each world's profile was trained through, as the report discloses it. */
const TRAINED_THROUGH: Record<PgoBenchWorld, string> = {
	ffi:
		'the library itself, in process under Deno through the bench’s own FFI wrapper — ' +
		'parse, parse_internal and format on every file',
	napi:
		'the addon itself — parse on every file and format --jobs 1 over the tree through the ' +
		'@fuzdev/tsv loader’s cli.js, the type strip and import records in process — the ' +
		'training the published addon gets'
};

/** What `run_pgo_bench` reaches. */
export interface PgoBenchDeps {
	/** The checkout: the pipeline runs in it, and `.pgo/` beneath it is read and written. */
	checkout: FsDir;
	/** The `node` the pipeline runs under, with the whole environment (module doc). */
	node: SpawnProgram;
	/** The host's platform triple, which names the pipeline's build of each library. */
	host_triple: string;
	/** `TSV_BUILD_FORCE=1`: rebuild whatever the stamp says. */
	force: boolean;
	/** The time the bench stamp records as `generated`. */
	now: () => Date;
	print: (line: string) => void;
	print_error: (line: string) => void;
}

/** The pipeline's argv, after `node`. */
export const pgo_bench_pipeline_args = (force: boolean): string[] => [
	'scripts/pgo/pgo.ts',
	'all',
	'--ffi',
	'--no-depth',
	...(force ? [] : ['--reuse']),
	'--dir',
	PGO_DIR
];

/** A result's exit code as a shell reports it: `128 + N` for a child signal `N` ended. The
 * pipeline runs with no timeout or abort signal, so it never ends any other way. */
const exit_code_of = (result: SpawnResult): number => {
	if (result.kind === 'exited') return result.code;
	if (result.kind === 'signaled') return signal_exit_code(result.signal);
	return result.signal === null ? (result.code ?? 1) : signal_exit_code(result.signal);
};

/** The sha256 of bytes a handle read, which hold their own memory, never a shared buffer. */
const sha256_hex = async (bytes: Uint8Array): Promise<string> =>
	Array.from(
		new Uint8Array(await crypto.subtle.digest('SHA-256', bytes as Uint8Array<ArrayBuffer>)),
		(b) => b.toString(16).padStart(2, '0')
	).join('');

/**
 * Runs the pipeline, then stages its two libraries and the bench stamp (module doc).
 * @returns the exit code: `0` once staged, `1` for a failed pipeline or a library the
 * pipeline's stamp doesn't name
 * @throws `FsError` - when the pipeline left no corpus or stamp to read, or a copy fails
 */
export const run_pgo_bench = async (deps: PgoBenchDeps): Promise<number> => {
	const pipeline = await deps.node.run(pgo_bench_pipeline_args(deps.force), {
		cwd: deps.checkout
	});
	if (pipeline.kind !== 'exited' || pipeline.code !== 0) {
		deps.print_error(
			`build:bench:pgo: the PGO pipeline failed (exit ${exit_code_of(pipeline)}); nothing staged`
		);
		return 1;
	}

	const pgo = deps.checkout.dir(PGO_DIR);
	const paths = pgo_paths('');
	const built = cargo_artifacts(paths.target('use'), deps.host_triple);
	const source: Record<PgoBenchWorld, string> = { ffi: built.ffi, napi: built.addon };
	const corpus = JSON.parse(await pgo.read_text(paths.corpus)) as { own: string[] };

	// the pipeline's own stamp names what its gates passed; the bench stamp may name nothing else
	const gated = JSON.parse(await pgo.read_text(paths.stamp)) as {
		artifacts: Record<string, { sha256: string; profile_sha256: string }>;
	};

	const bench_stamp = `${PGO_BENCH_SUBDIR}/${PGO_BENCH_STAMP_NAME}`;
	await pgo.remove(bench_stamp, { missing_ok: true });
	await pgo.mkdir(PGO_BENCH_SUBDIR, { recursive: true });
	const artifacts = {} as PgoBenchStamp['artifacts'];
	for (const world of PGO_BENCH_WORLDS) {
		const dest = `${PGO_BENCH_SUBDIR}/${pgo_bench_library_name(world)}`;
		// a fresh file rather than an overwrite in place: a process that has the old library
		// loaded keeps its own mapping
		await pgo.remove(dest, { missing_ok: true });
		await pgo.copy(source[world], dest);
		const info = await pgo.stat(source[world]);
		if (info === null)
			throw new Error(`${PGO_DIR}/${source[world]} was removed while it was copied`);
		await pgo.set_mode(dest, info.mode);
		artifacts[world] = {
			sha256: await sha256_hex(await pgo.read(dest)),
			profile_sha256: await sha256_hex(await pgo.read(paths.profdata(world))),
			trained_through: TRAINED_THROUGH[world]
		};
		const passed = gated.artifacts[world];
		if (
			passed === undefined ||
			passed.sha256 !== artifacts[world].sha256 ||
			passed.profile_sha256 !== artifacts[world].profile_sha256
		) {
			deps.print_error(
				`build:bench:pgo: the ${world} library or profile on disk is not what ` +
					`${PGO_DIR}/${paths.stamp} records the gates passing — nothing staged`
			);
			await pgo.remove(dest, { missing_ok: true });
			return 1;
		}
	}
	const stamp: PgoBenchStamp = {
		version: PGO_BENCH_STAMP_VERSION,
		generated: deps.now().toISOString(),
		training: {
			origin: 'own',
			collections: corpus.own,
			snapshot_tree: GATE_CHECKOUT_IDS[CORPORA_ROOT]!.hash
		},
		artifacts
	};
	await pgo.write(bench_stamp, JSON.stringify(stamp, null, '\t') + '\n');
	deps.print(`build:bench:pgo: staged the PGO FFI library and N-API addon in ${PGO_BENCH_DIR}`);
	return 0;
};

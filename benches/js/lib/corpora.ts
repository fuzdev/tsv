/**
 * The real-code snapshot every `real` / `framework` / `third_party` corpus entry reads:
 * where the `fuzdev/corpora` checkout sits, which tree inside it consumers walk, and
 * the reader for its recipe (`manifest.json`) — the recipe is also where a collection's
 * subpaths are spelled, so the corpus entries are derived from it rather than restated.
 * One spelling of the path, so the loader, the report's source links, the styles
 * harvest and its stamp, the checkout pin and the doctor cannot name the snapshot three
 * different ways.
 *
 * Node builtins only, so `scripts/` and the node-modules-free bench core can import it.
 */

import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

/** The sibling checkout — also the key `GATE_CHECKOUT_IDS` pins it under. */
export const CORPORA_ROOT = '../corpora';

/** The snapshot repo's canonical URL — what a missing-checkout remedy tells the reader to clone. */
export const CORPORA_URL = 'https://github.com/fuzdev/corpora';

/**
 * The subtree that IS the corpus, repo-relative — what the pin and the styles stamp
 * hash (`git rev-parse HEAD:collections`), so a tooling or doc commit in the snapshot
 * repo moves neither.
 */
export const CORPORA_TREE = 'collections';

/**
 * The one tree consumers walk. A corpus entry names a collection's upstream-relative
 * subpath beneath it (`collections/kit/packages/kit/src`), never the repo root, so the
 * walk sees only corpus files — the snapshot's own scaffolding lives outside it.
 */
export const CORPORA_COLLECTIONS = `${CORPORA_ROOT}/${CORPORA_TREE}`;

/** The snapshot's recipe — one entry per collection, naming the upstream + the commit it vendored. */
export const CORPORA_MANIFEST = `${CORPORA_ROOT}/manifest.json`;

/** The manifest format this reader understands; a bump upstream is a change here. */
export const CORPORA_MANIFEST_VERSION = 1;

/** Whether a corpus entry path reads a snapshot collection. */
export function is_collection_path(path: string): boolean {
	return path.startsWith(`${CORPORA_COLLECTIONS}/`);
}

/**
 * A collection path split into the collection's name and its upstream-relative
 * subpath: `../corpora/collections/kit/packages/kit/src` → `kit` + `packages/kit/src`.
 * `null` for a path outside the snapshot.
 */
export function split_collection_path(path: string): { name: string; subpath: string } | null {
	if (!is_collection_path(path)) return null;
	const rest = path.slice(CORPORA_COLLECTIONS.length + 1);
	const slash = rest.indexOf('/');
	return slash === -1
		? { name: rest, subpath: '' }
		: { name: rest.slice(0, slash), subpath: rest.slice(slash + 1) };
}

/** One collection's provenance, extent and filter, as far as consumers here need them. */
export interface CorporaCollection {
	/** The upstream's canonical https GitHub URL. */
	url: string;
	/** The upstream commit the collection was vendored at (full SHA). */
	commit: string;
	/**
	 * The upstream-relative directories the snapshot vendors, in manifest order — what a
	 * consumer walks (`collections/<name>/<subpath>`), and what a live working tree of
	 * the same repo is diffed under. Spelled once, here, so a corpus entry never restates
	 * an upstream's layout (`corpus.ts` derives its entries from this list).
	 */
	subpaths: string[];
	/**
	 * Upstream-relative path prefixes the snapshot leaves out — the upstream's own test
	 * fixtures and vendored bundles. What a live working tree of the same repo must drop
	 * too before it can stand in for the collection (`live_diff_files`).
	 */
	exclude: string[];
}

/** The manifest's shape, as far as this reader needs it — raw JSON, checked field by field. */
interface CorporaManifest {
	version: unknown;
	collections: unknown;
}

/** Whether `path` equals `prefix` or lies beneath it (both `/`-separated, relative). */
export function is_under(path: string, prefix: string): boolean {
	return path === prefix || path.startsWith(`${prefix}/`);
}

/**
 * The fields this reader takes from one manifest collection, or `null` when any is
 * missing or mistyped. Checked rather than trusted: the manifest is another repo's
 * file, and a shape change there that forgot the version bump must read as a loud
 * refusal here — not as `undefined` upstream links and an empty `exclude` that lets
 * the upstream's fixtures into the live diff.
 */
function read_collection(raw: unknown): ({ name: string } & CorporaCollection) | null {
	if (typeof raw !== 'object' || raw === null) return null;
	const { name, url, commit, subpaths, exclude } = raw as Record<string, unknown>;
	if (typeof name !== 'string' || typeof url !== 'string' || typeof commit !== 'string') {
		return null;
	}
	const is_string_list = (v: unknown): v is string[] =>
		Array.isArray(v) && v.every((e) => typeof e === 'string');
	// A collection with no subpath vendors nothing a consumer could walk.
	if (!is_string_list(subpaths) || subpaths.length === 0) return null;
	const excludes = exclude === undefined ? [] : exclude;
	if (!is_string_list(excludes)) return null;
	return { name, url, commit, subpaths, exclude: excludes };
}

/**
 * What reading the manifest found. `absent` is no file at all — the snapshot is not
 * checked out, which the corpus loader discloses in its own terms; `unreadable` is a
 * file this reader can't take (a version it doesn't know, or a collection missing a
 * field it needs) — drift, not absence, warned once here and refused by every consumer
 * that would derive a corpus from it.
 */
export type CorporaManifestRead =
	| { status: 'ok'; collections: Map<string, CorporaCollection> }
	| { status: 'absent' }
	| { status: 'unreadable'; reason: string };

let manifest_promise: Promise<CorporaManifestRead> | undefined;

/** The snapshot's collections by name, read once — see `CorporaManifestRead` for the two ways there are none. */
export function load_corpora_manifest(): Promise<CorporaManifestRead> {
	manifest_promise ??= (async (): Promise<CorporaManifestRead> => {
		let text: string;
		try {
			text = await readFile(resolve(CORPORA_MANIFEST), 'utf8');
		} catch {
			return { status: 'absent' };
		}
		const unreadable = (reason: string): CorporaManifestRead => {
			console.warn(
				`  ⚠ ${CORPORA_MANIFEST} ${reason} — no corpus entry can be derived from it, and ` +
					'sources link to the snapshot repo rather than their upstreams'
			);
			return { status: 'unreadable', reason };
		};
		let json: unknown;
		try {
			json = JSON.parse(text);
		} catch (e) {
			return unreadable(`is not JSON (${e})`);
		}
		// `JSON.parse` accepts `null` and scalars too; those must read as unreadable
		// rather than throw out of the field checks below.
		if (typeof json !== 'object' || json === null) return unreadable('is not a JSON object');
		const parsed = json as CorporaManifest;
		if (parsed.version !== CORPORA_MANIFEST_VERSION) {
			return unreadable(
				`is version ${parsed.version}; this reader knows ${CORPORA_MANIFEST_VERSION}`
			);
		}
		if (!Array.isArray(parsed.collections)) return unreadable('has no `collections` array');
		const collections = new Map<string, CorporaCollection>();
		for (const raw of parsed.collections) {
			const c = read_collection(raw);
			if (c === null) {
				return unreadable(
					`has a collection this reader can't read (${JSON.stringify(raw).slice(0, 80)}): ` +
						'its shape moved without a version bump'
				);
			}
			const { name, ...collection } = c;
			collections.set(name, collection);
		}
		return { status: 'ok', collections };
	})();
	return manifest_promise;
}

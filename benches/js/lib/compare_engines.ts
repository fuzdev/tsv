/**
 * Loads the engines a comparison run grades with (`compare_cli.ts`' `CompareImplementations`):
 * the canonical oracles (prettier, svelte/compiler, acorn) and tsv's N-API addon at one cargo
 * profile, behind the artifact-freshness guard. The comparison tools' roots call it; the tools
 * themselves take the result as a dep.
 *
 * @module
 */

import { CanonicalImplementation } from './canonical.ts';
import { check_artifact_freshness, native_artifact_check } from './check_artifact_freshness.ts';
import { check_node_modules } from './check_node_modules.ts';
import type { CompareImplementations } from './compare_cli.ts';
import { NapiImplementation } from './napi.ts';
import { napi_library_path } from './tsv_artifacts.ts';
import { load_all_versions } from './versions.ts';

/** What `init_compare_implementations` loads. */
export interface CompareEnginesOptions {
	/** The cargo profile whose plain N-API addon is loaded and guarded (`napi_library_path`). */
	profile: string;
}

/**
 * Initialize the canonical and native implementations behind the artifact-freshness guard,
 * printing a profile-aware rebuild hint and returning `null` when one fails to load. The
 * guard and the loaded addon come from the one `profile`, so they can't disagree. A missing
 * or stale bench install, or a stale addon, exits the process from its own check.
 */
export async function init_compare_implementations(
	options: CompareEnginesOptions
): Promise<CompareImplementations | null> {
	// The canonical impls resolve from benches/js/node_modules — check it exists
	// (and isn't stale) up front so the failure is the installer hint, not an
	// opaque module-resolution error mid-init.
	await check_node_modules();

	// Refuse to measure a stale binary (the `:run` task variants skip the
	// rebuild). Only the native addon: these tools run no WASM, so the bundle's
	// freshness is not their business. Override with BENCH_STALE_OK=1. Held in a
	// const because the init failure below quotes the same rebuild hint — one
	// answer to "how do I rebuild this artifact", whether it was stale or unloadable.
	const native_check = native_artifact_check(options.profile);
	await check_artifact_freshness([native_check]);

	const versions = await load_all_versions();
	const canonical = new CanonicalImplementation(versions.canonical);
	const native = new NapiImplementation(napi_library_path(options.profile));

	try {
		await canonical.init();
	} catch (e) {
		console.error(`Failed to initialize canonical implementations: ${e}`);
		return null;
	}

	try {
		await native.init();
	} catch (e) {
		console.error(`Failed to initialize native implementation: ${e}`);
		console.error(`Run: ${native_check.rebuild}`);
		canonical.dispose();
		return null;
	}

	return {
		canonical,
		native,
		dispose: () => {
			canonical.dispose();
			native.dispose();
		}
	};
}

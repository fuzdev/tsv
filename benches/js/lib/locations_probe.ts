/**
 * The shared "did `locations: false` actually TAKE" probe, for tsv's three
 * front-ends.
 *
 * The no-locations rows (`tsv-json-no-locations`, `tsv-wasm-json-no-locations`)
 * publish the span-only wire, and nothing in a timed sweep can tell that wire from
 * the loc-bearing one: both are a successful parse returning an AST, so an artifact
 * that ignores the request is timed as usual and published under the wrong label —
 * a with-locations parse in the no-locations row, and a payload-matched comparison
 * that no longer matches. A WASM bundle built before the options bag existed does
 * exactly this: its parse exports take one argument, so `{locations: false}` is
 * dropped by the glue without a throw.
 *
 * The artifact-freshness guard refuses such a bundle by default
 * (`lib/check_artifact_freshness.ts`), and this does not replace it: it is what
 * stands behind `BENCH_STALE_OK=1`, which downgrades stale to a warning, and behind
 * a binding regression in an artifact that is perfectly fresh.
 *
 * So the check is BEHAVIORAL, like `lib/reject_probe.ts` and
 * `lib/format_config_probe.ts`: parse one fixed source through the row's own call
 * and read the answer back. ONE probe and one grader for all three bindings, on the
 * rule those modules follow — a second spelling of the same question is free to
 * drift into asking a different one. The native bindings select the wire by a
 * separate export rather than an option, so the WASM failure above cannot happen to
 * them in that form; they are probed anyway, since an export wired to the wrong
 * writer is the same mislabelled row.
 *
 * @module
 */

import { probe_goals } from './reject_probe.ts';
import type { Language, ParseGoal } from './types.ts';

/** The languages with a benched no-locations row — none for CSS (`implementations.ts`). */
export type LocationsLanguage = Exclude<Language, 'css'>;

/**
 * One source per language whose loc-bearing AST carries `loc` — valid at both
 * TypeScript goals, and free of any identifier that could put a `loc` KEY in the
 * tree by another route.
 */
export const LOCATIONS_PROBE_SOURCES: Readonly<Record<LocationsLanguage, string>> = {
	svelte: '<script>let x = 1;</script>\n<p>{x}</p>',
	typescript: 'const x = 1;'
};

/**
 * Count the `loc` keys anywhere in `ast`.
 *
 * Keys, not values: a source string or identifier spelled `loc` is a value in the
 * wire and never counts.
 */
export function count_loc_keys(ast: unknown): number {
	if (typeof ast !== 'object' || ast === null) return 0;
	if (Array.isArray(ast)) {
		let count = 0;
		for (const item of ast) count += count_loc_keys(item);
		return count;
	}
	let count = 0;
	for (const [key, value] of Object.entries(ast)) {
		if (key === 'loc') count++;
		count += count_loc_keys(value);
	}
	return count;
}

/** The artifact a binding loaded, for the failure message. */
export interface LocationsProbeArtifact {
	/** The file the binding opened. */
	path: string;
	/** The command that rebuilds it. */
	rebuild: string;
}

/**
 * Grade one probe: `without` (the no-locations parse) must carry no `loc`, and
 * `with_locations` (its loc-bearing sibling, same source) must carry some.
 *
 * The second half is what keeps the first from passing vacuously — a wire that
 * stopped emitting `loc` at all, or a probe source that never had any, would read
 * as "dropped" on every run.
 *
 * @param binding - the row-facing name, so the throw names which one failed
 * @param operation - the probed call, for the message
 * @param artifact - what the binding loaded and how to rebuild it
 * @param with_locations - the loc-bearing parse of the probe source
 * @param without - the no-locations parse of the same source
 * @throws Error when `without` carries `loc`, or when `with_locations` carries none
 */
export function assert_locations_dropped(
	binding: string,
	operation: string,
	artifact: LocationsProbeArtifact,
	with_locations: unknown,
	without: unknown
): void {
	const kept = count_loc_keys(without);
	if (kept > 0) {
		throw new Error(
			`${binding}: ${operation}() returned an AST carrying ${kept} \`loc\` ` +
				`key${kept === 1 ? '' : 's'} — the artifact at ${artifact.path} still emits \`loc\` on its ` +
				`no-locations parse, so the no-locations row would time the loc-bearing wire ` +
				`under the wrong label. Rebuild it: '${artifact.rebuild}'. See lib/locations_probe.ts.`
		);
	}
	if (count_loc_keys(with_locations) === 0) {
		throw new Error(
			`${binding}: the loc-bearing sibling of ${operation}() returned an AST with no \`loc\` ` +
				`at all (artifact: ${artifact.path}) — the probe no longer discriminates, so it ` +
				`cannot say whether the no-locations parse drops \`loc\`. See lib/locations_probe.ts.`
		);
	}
}

/** The two parse operations the probe compares. */
export interface LocationsProbeTarget {
	parse(source: string, language: Language, goal?: ParseGoal): unknown;
	parse_no_locations(source: string, language: Language, goal?: ParseGoal): unknown;
}

/**
 * Assert `impl`'s no-locations parse really drops `loc`, in every language and at
 * every goal it publishes such a row for, failing the impl loudly when it doesn't.
 *
 * Called from the wrapper's `init()`, once, outside any timed loop, through the
 * wrapper's OWN methods so the probe runs the call the timed row makes. tsv's
 * bindings are `init_required`, so a failure stops the run rather than dropping a
 * row — the posture `assert_binding_reports_rejection` takes, for the same reason.
 *
 * @param binding - the row-facing name, so the throw names which one failed
 * @param artifact - what the binding loaded and how to rebuild it
 * @param impl - the binding, called through its own methods so each keeps its receiver
 */
export function assert_binding_drops_locations(
	binding: string,
	artifact: LocationsProbeArtifact,
	impl: LocationsProbeTarget
): void {
	for (const language of Object.keys(LOCATIONS_PROBE_SOURCES) as Array<LocationsLanguage>) {
		const source = LOCATIONS_PROBE_SOURCES[language];
		for (const goal of probe_goals(language)) {
			assert_locations_dropped(
				binding,
				`parse_no_locations[${language}${goal ? `, ${goal}` : ''}]`,
				artifact,
				impl.parse(source, language, goal),
				impl.parse_no_locations(source, language, goal)
			);
		}
	}
}

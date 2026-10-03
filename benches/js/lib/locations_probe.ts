/**
 * The shared "is the parse wire still SPAN-ONLY" probe, for tsv's three front-ends.
 *
 * Every tsv binding emits one parse wire: `start`/`end` offsets, no per-node `loc`
 * (Svelte also no `name_loc`). The bench's span rows (`tsv-json-no-locations`,
 * `tsv-wasm-json-no-locations`) publish that wire, and their `+reconstruct` siblings
 * time what `{locations: true}` adds over it — and nothing in a timed sweep can tell
 * that wire from a loc-bearing one: both are a successful parse returning an AST, so an
 * artifact still emitting `loc` is timed as usual and published under the span label,
 * with a reconstruction running over a tree that already had `loc`. A binding built
 * before the bindings went span-only does exactly this.
 *
 * The artifact-freshness guard refuses such a build by default
 * (`lib/check_artifact_freshness.ts`), and this does not replace it: it is what stands
 * behind `BENCH_STALE_OK=1`, which downgrades stale to a warning, and behind a binding
 * regression in an artifact that is perfectly fresh.
 *
 * So the check is BEHAVIORAL, like `lib/reject_probe.ts` and
 * `lib/format_config_probe.ts`: parse one fixed source per language through the row's
 * own call and read the answer back. ONE probe and one grader for all three bindings,
 * on the rule those modules follow — a second spelling of the same question is free to
 * drift into asking a different one.
 *
 * @module
 */

import { probe_goals } from './reject_probe.ts';
import { type Language, LANGUAGES, type ParseGoal } from './types.ts';

/**
 * One source per language whose loc-bearing wire would carry `loc` on every node —
 * valid at both TypeScript goals, and free of any identifier that could put a `loc`
 * KEY in the tree by another route.
 */
export const LOCATIONS_PROBE_SOURCES: Readonly<Record<Language, string>> = {
	svelte: '<script>let x = 1;</script>\n<p>{x}</p>',
	typescript: 'const x = 1;',
	css: 'a { color: red; }'
};

/** The keys only a loc-bearing wire carries. */
const LOCATION_KEYS: ReadonlySet<string> = new Set(['loc', 'name_loc']);

/**
 * Count the `loc` and `name_loc` keys anywhere in `ast`.
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
		if (LOCATION_KEYS.has(key)) count++;
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
 * Grade one probe: `ast` must carry no `loc` / `name_loc`, and its root must carry
 * numeric `start` / `end`.
 *
 * The second half is what keeps the first from passing vacuously — an error object, an
 * empty result, or a wire that stopped emitting offsets would carry no `loc` either.
 *
 * @param binding - the row-facing name, so the throw names which one failed
 * @param operation - the probed call, for the message
 * @param artifact - what the binding loaded and how to rebuild it
 * @param ast - the parse of the probe source
 * @throws Error when `ast` carries a location key, or its root no numeric offsets
 */
export function assert_span_only(
	binding: string,
	operation: string,
	artifact: LocationsProbeArtifact,
	ast: unknown
): void {
	const kept = count_loc_keys(ast);
	if (kept > 0) {
		throw new Error(
			`${binding}: ${operation}() returned an AST carrying ${kept} \`loc\` / \`name_loc\` ` +
				`key${kept === 1 ? '' : 's'} — the artifact at ${artifact.path} still emits the ` +
				`loc-bearing wire, so the span rows would time it under the wrong label. Rebuild ` +
				`it: '${artifact.rebuild}'. See lib/locations_probe.ts.`
		);
	}
	const root = ast as { start?: unknown; end?: unknown } | null;
	if (typeof root?.start !== 'number' || typeof root?.end !== 'number') {
		throw new Error(
			`${binding}: ${operation}() returned an AST whose root has no numeric \`start\`/\`end\` ` +
				`(artifact: ${artifact.path}) — the probe no longer discriminates, so it cannot ` +
				`say the wire is the span-only one. See lib/locations_probe.ts.`
		);
	}
}

/** The parse operation the probe grades. */
export interface LocationsProbeTarget {
	parse(source: string, language: Language, goal?: ParseGoal): unknown;
}

/**
 * Assert `impl`'s parse emits the span-only wire, in every language it parses and at
 * every goal, failing the impl loudly when it doesn't.
 *
 * Called from the wrapper's `init()`, once, outside any timed loop, through the
 * wrapper's OWN method so the probe runs the call the timed rows make. tsv's bindings
 * are `init_required`, so a failure stops the run rather than dropping a row — the
 * posture `assert_binding_reports_rejection` takes, for the same reason.
 *
 * @param binding - the row-facing name, so the throw names which one failed
 * @param artifact - what the binding loaded and how to rebuild it
 * @param impl - the binding, called through its own method so it keeps its receiver
 * @param languages - the languages the binding parses
 */
export function assert_binding_emits_span_only(
	binding: string,
	artifact: LocationsProbeArtifact,
	impl: LocationsProbeTarget,
	languages: ReadonlyArray<Language> = LANGUAGES
): void {
	for (const language of languages) {
		const source = LOCATIONS_PROBE_SOURCES[language];
		for (const goal of probe_goals(language)) {
			assert_span_only(
				binding,
				`parse[${language}${goal ? `, ${goal}` : ''}]`,
				artifact,
				impl.parse(source, language, goal)
			);
		}
	}
}

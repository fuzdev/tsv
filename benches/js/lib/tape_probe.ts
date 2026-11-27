/**
 * The shared "does the parse run through the TAPE" probe, for tsv's three front-ends.
 *
 * The packages' `parse_<lang>` takes the tape route — the engine's raw `parse_tape_<lang>`
 * lays the span-only wire down as the binary tape and the generated decoder builds the tree
 * (`docs/tape_format.md`) — and the bench's `tsv` / `tsv-wasm` rows and their `+locations` siblings
 * time that route under the packages' names. Nothing in a timed sweep can tell it from another
 * route returning the same tree. The bench builds its published tables by hand
 * (`lib/tsv_api.ts` `publish_parse`, over each wrapper's engine table), so a wiring that skipped
 * the tape export would time something else under its name; an artifact whose tape the decoder
 * cannot read (a stale build written against another schema or shape list) would fail every
 * row's first call in a message that names neither the artifact nor the fix.
 *
 * So the check is BEHAVIORAL, like `lib/locations_probe.ts`, in three parts per language:
 *
 * - **routing** — an untimed twin of the published table, built by the same function over
 *   the engine with its tape export counted, must run the tape export once, for the default
 *   parse and for `{locations: true}`, and the tape
 *   export must hand back a `Uint32Array` (words, so the decoder views them in place, 4-aligned
 *   by type). The twin, not the timed table, because counting the timed one would mean
 *   wrapping the exports the rows time; and only where asked (`twin`), since a twin's closures
 *   share their creation sites' type feedback with the timed ones — pre-flight asks it, a timed
 *   row's own process does not. That `bind` builds the timed table through the same function
 *   is `tape_probe_test.ts`'s to pin, over counted exports.
 * - **the tree** — where the binding emits the JSON wire (the C FFI's `tsv_parse_<lang>`; the
 *   npm engines emit none, and their decode is held to the native CLI's wire by the package
 *   suites), the row's own call must return what `JSON.parse` makes of that wire for the same
 *   source, compared as `JSON.stringify` text so KEY ORDER counts (the decoder promises the
 *   wire's order, which a structural compare would not see).
 *   Pre-flight only, like the routing half: calling the JSON export would send a second
 *   engine function through the wrapper's own call sites (the FFI wrapper's one native call
 *   site serves every export) ahead of a timed row's clock, and pre-flight has compared the
 *   same artifact's bytes.
 * - **the wording** — a tape the decoder refuses is rethrown naming the artifact and its
 *   rebuild task, the cause kept — in a timed row's own process too, where the row's own
 *   call is the only one made.
 *
 * @module
 */

import { type LocationsProbeArtifact, LOCATIONS_PROBE_SOURCES } from './locations_probe.ts';
import { probe_goals } from './reject_probe.ts';
import type { Language, ParseGoal } from './types.ts';

/** A flat engine export, as `lib/tsv_api.ts` declares it (restated: this module imports no
 * value from there, which imports this). */
type EngineFn<T> = (source: string, source_type?: string) => T;

/** The parse export a published table is built over, and the JSON wire a binding may emit
 * beside it (the C FFI's alone). */
export interface TapeProbeEngine {
	parse_tape: Record<Language, EngineFn<unknown>>;
	parse_json?: Record<Language, EngineFn<unknown>>;
}

/** What builds a published parse table over an engine — `lib/tsv_api.ts` `publish_parse`. */
export type PublishParse = (
	engine: TapeProbeEngine
) => Record<string, (source: string, options?: object) => unknown>;

/** The binding the probe grades: its engine, and the row's own default parse. */
export interface TapeProbeTarget {
	readonly engine: TapeProbeEngine;
	parse(source: string, language: Language, goal?: ParseGoal): unknown;
}

/** The message `tape_engine.js` throws for a tape the decoder refuses. */
const UNDECODABLE = 'internal error: the engine returned a tape that does not decode';

/**
 * Assert `impl`'s default parse runs through the tape and, where the binding emits the JSON
 * wire, returns its tree, in every listed language and at every goal, failing loudly when it
 * doesn't.
 *
 * Called from the wrapper's `init()`, once, outside any timed loop. tsv's bindings are
 * `init_required`, so a failure stops the run — the posture `assert_binding_emits_span_only`
 * takes, for the same reason.
 *
 * @param binding - the row-facing name, so the throw names which one failed
 * @param artifact - what the binding loaded and how to rebuild it
 * @param impl - the binding, called through its own method so it keeps its receiver
 * @param languages - the languages to probe
 * @param twin - the function that builds the timed table, to build the routing twin with;
 * 	`null` skips the routing half and the JSON comparison (a timed row's own process)
 * @throws Error when the twin does not route through the tape, the tree differs from the
 * 	JSON wire's, or the tape does not decode
 */
export function assert_binding_parses_through_tape(
	binding: string,
	artifact: LocationsProbeArtifact,
	impl: TapeProbeTarget,
	languages: ReadonlyArray<Language>,
	twin: PublishParse | null
): void {
	if (twin !== null) assert_routing(binding, artifact, impl.engine, languages, twin);
	for (const language of languages) {
		const source = LOCATIONS_PROBE_SOURCES[language];
		for (const goal of probe_goals(language)) {
			const call = `parse[${language}${goal ? `, ${goal}` : ''}]`;
			let tree: unknown;
			try {
				tree = impl.parse(source, language, goal);
			} catch (e) {
				if (e instanceof Error && e.message.startsWith(UNDECODABLE)) {
					throw new Error(
						`${binding}: ${call}() returned a tape the generated decoder cannot read ` +
							`(${(e.cause as Error | undefined)?.message ?? 'no cause'}) — the artifact at ` +
							`${artifact.path} was built against another schema or shape list than ` +
							`crates/tsv_wasm/npm/tape_decode.js. Rebuild it: '${artifact.rebuild}'. ` +
							`See lib/tape_probe.ts.`,
						{ cause: e }
					);
				}
				throw e;
			}
			const parse_json = impl.engine.parse_json?.[language];
			if (twin === null || parse_json === undefined) continue;
			const wire = parse_json(source, goal);
			if (typeof wire !== 'string') {
				throw new Error(
					`${binding}: the ${language} JSON wire export returned no wire for the ` +
						`probe source (artifact: ${artifact.path}) — the tape probe has nothing to ` +
						`compare. ` +
						`See lib/tape_probe.ts.`
				);
			}
			if (JSON.stringify(tree) !== JSON.stringify(JSON.parse(wire))) {
				throw new Error(
					`${binding}: ${call}() returned a tree that differs from JSON.parse of ` +
						`the ${language} JSON wire (keys, values or key order) — the tape route ` +
						`and the JSON wire disagree in the artifact at ${artifact.path}, so the tsv rows ` +
						`would time a tree the packages' parse does not return. Rebuild it: ` +
						`'${artifact.rebuild}'; if it persists, the writer's tape copy or the decoder is ` +
						`wrong (npm run check:tape). See lib/tape_probe.ts.`
				);
			}
		}
	}
}

/** The routing half: an untimed twin of the published table, its two exports counted. */
function assert_routing(
	binding: string,
	artifact: LocationsProbeArtifact,
	engine: TapeProbeEngine,
	languages: ReadonlyArray<Language>,
	twin: PublishParse
): void {
	let tapes = 0;
	let returned: unknown = undefined;
	// the whole table, every language: the facade builds a function per language it is handed
	const counted = (
		of: (language: Language) => EngineFn<unknown>
	): Record<Language, EngineFn<unknown>> => ({
		svelte: of('svelte'),
		typescript: of('typescript'),
		css: of('css')
	});
	const table = twin({
		parse_tape: counted((language) => (source, source_type) => {
			tapes++;
			returned = engine.parse_tape[language](source, source_type);
			return returned;
		})
	});
	for (const language of languages) {
		const parse = table[`parse_${language}`];
		for (const options of [undefined, { locations: true }]) {
			const call = `parse_${language}(${options ? '{locations: true}' : ''})`;
			if (parse === undefined) {
				throw new Error(`${binding}: the published table has no parse_${language}`);
			}
			tapes = 0;
			returned = undefined;
			const result = parse(LOCATIONS_PROBE_SOURCES[language], options) as { ok?: unknown };
			if (tapes !== 1) {
				throw new Error(
					`${binding}: ${call} ran the tape export ${tapes} time${tapes === 1 ? '' : 's'} ` +
						`— the bench's published table ` +
						`(lib/tsv_api.ts publish_parse) no longer takes the tape route the packages' ` +
						`./parse entries take, so the tsv rows would time another route under its ` +
						`name. See lib/tape_probe.ts.`
				);
			}
			if (!(returned instanceof Uint32Array)) {
				throw new Error(
					`${binding}: parse_tape_${language} returned ${describe(returned)} where the tape ` +
						`route reads a Uint32Array of the tape's words (artifact: ${artifact.path}). ` +
						`See lib/tape_probe.ts.`
				);
			}
			if (result?.ok !== true) {
				throw new Error(
					`${binding}: ${call} over the routing twin returned no AST for the probe source ` +
						`(artifact: ${artifact.path}). See lib/tape_probe.ts.`
				);
			}
		}
	}
}

/** A value's kind, for a message. */
const describe = (value: unknown): string =>
	value === null
		? 'null'
		: typeof value === 'object'
			? `a ${(value as object).constructor?.name ?? 'object'}`
			: `a ${typeof value}`;

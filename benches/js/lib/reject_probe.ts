/**
 * The shared "does this binding still REPORT a rejection" probe, for tsv's three
 * front-ends.
 *
 * A row's coverage number is only as good as its error surface. The failure mode is
 * not that a tool crashes — that is loud — but that it stops SAYING it refused, at
 * which point every file it rejected counts as processed and the row publishes a
 * fabricated 100%. The oxc WASI binding did exactly this through a consume-once
 * `errors` getter (benches/js/CLAUDE.md §Known Issues), which is why
 * `lib/oxc.ts`'s `assert_oxc_rejects_invalid` exists.
 *
 * tsv's own bindings share that shape, and one of them has no other guard:
 *
 * - **FFI** returns a payload either way, so `NativeImplementation.call_ffi` decides
 *   "rejected" from the `out_status` word `tsv_ffi` writes beside `out_len`. Break
 *   that — a caller reading the wrong slot, an export that stops writing — and
 *   `format` silently returns the error JSON *as the formatted output*, scoring every
 *   refusal as a success. The second of those is closed structurally rather than by
 *   this probe: `call_ffi` seeds a `STATUS_UNWRITTEN` sentinel before every call, so
 *   an unwritten status throws instead of inheriting the last call's verdict. This
 *   probe is what covers the first, and it covers it BEHAVIORALLY — the only way to
 *   ask whether a refusal still arrives as one.
 * - **N-API** and **WASM** throw natively today. Probed anyway, and for the reason
 *   the yuku wrapper gives for probing an option whose loss would be loud: a shared
 *   question asked at one site and not the others is free to drift, and the cost
 *   here is one call per row at init.
 *
 * The status word REPLACED a content sniff — a `startsWith('{"error"')` test on the
 * decoded payload, sound only because tsv normalizes strings to single quotes, i.e. a
 * correctness dependency on a *style* setting over a channel carrying arbitrary
 * formatted source. This probe is what made that fragility legible, and it guards the
 * replacement on exactly the same terms.
 *
 * The existing detector for the FFI case is the accept-set half of
 * `check_variant_parity` (native accepts, wasm rejects → a divergence). It is
 * warning-only, and on the perf corpus — where tsv parses and formats every file —
 * no file exercises it at all, so it can be green while the surface is broken.
 *
 * ONE probe and one grader for all three bindings rather than a copy per wrapper,
 * on the rule `lib/format_config_probe.ts` follows: a second spelling of the same
 * question is free to drift into asking a different one. Which is also why the
 * grader takes the IMPL and walks every operation itself — a per-binding call list
 * would let a binding quietly probe fewer rows than it publishes.
 *
 * @module
 */

import type { Language } from './types.ts';

/**
 * A source every tsv entry point must refuse: a Svelte component whose `<script>`
 * holds an expression-position `;`.
 *
 * Svelte rather than bare TypeScript because it drives the embedding path too, and
 * shallow enough that no grammar change makes it legal. The refusal this probes is
 * language-independent — `tsv_ffi` writes its status through one function
 * (`bytes_to_ptr`) for every export, and the other two bindings throw — so one
 * language proves the mechanism for all of them.
 */
const REJECT_PROBE_SOURCE = '<script>let x = ;</script>';

/** The operations a tsv binding exposes, each of which is a published row. */
export interface RejectProbeTarget {
	parse(source: string, language: Language): unknown;
	parse_internal(source: string, language: Language): void;
	parse_no_locations(source: string, language: Language): unknown;
	format(source: string, language: Language): string;
}

/**
 * Assert every operation `binding` publishes still THROWS on `REJECT_PROBE_SOURCE`,
 * failing the impl loudly when one of them doesn't.
 *
 * Called from the wrapper's `init()`. tsv's bindings are `init_required`, so this
 * stops the run rather than dropping a row — correct, and the same rule the
 * optional impls follow: a failed self-check withdraws what it contaminates, and
 * for the subject of the benchmark that is every number the run would publish.
 *
 * Every operation rather than one, because a binding can lose its refusal on one
 * export and keep it on the others: each is a separate generated entry point, and
 * `parse_internal` is the one whose payload carries no tell at all — it is empty on
 * success, so nothing but the status distinguishes it from a refusal.
 *
 * @param binding - the row-facing name, so the throw names which one failed
 * @param impl - the binding, called through its own methods so each keeps its receiver
 */
export function assert_binding_reports_rejection(binding: string, impl: RejectProbeTarget): void {
	const operations: ReadonlyArray<[string, () => unknown]> = [
		['parse', () => impl.parse(REJECT_PROBE_SOURCE, 'svelte')],
		['parse_internal', () => impl.parse_internal(REJECT_PROBE_SOURCE, 'svelte')],
		['parse_no_locations', () => impl.parse_no_locations(REJECT_PROBE_SOURCE, 'svelte')],
		['format', () => impl.format(REJECT_PROBE_SOURCE, 'svelte')]
	];
	for (const [operation, run] of operations) {
		let accepted = false;
		try {
			run();
			accepted = true;
		} catch {
			// The expected outcome: the binding surfaced the refusal.
		}
		if (accepted) {
			throw new Error(
				`${binding}: ${operation}() ACCEPTED a source tsv rejects — the binding's error ` +
					`surface no longer reports a refusal, so every rejected file would count as ` +
					`processed and this row's coverage would be fabricated. See lib/reject_probe.ts.`
			);
		}
	}
}

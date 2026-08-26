/**
 * Input injection for the parse wire — the manufactured-input half of
 * `corpus_compare_parse.ts`.
 *
 * **Why this exists.** Every injection/fuzz audit in the repo grades a
 * *formatter-side, self-referential* property: gap/blank injection check formatting,
 * the fuzzer checks no-panic + idempotency + reparse, round-trip checks that output
 * reparses. The wire-vs-canonical comparison is the opposite — a real external oracle
 * — but it only ever ran over inputs someone had already written: committed fixtures
 * and real repos. Nothing manufactured an input and graded the resulting *wire*.
 *
 * That gap is not theoretical. A Svelte block binding's `: T` is read by its own acorn
 * parse, and both the line seed it needs and the span it is anchored at were wrong for
 * any spelling that put whitespace between the binding and its colon — invisible to
 * 9441 fixtures and every real repo, because everyone writes `x: T`. The comparison
 * catches both the instant such an input exists. This module makes them exist.
 *
 * **Two families, because there are two kinds of claim to break.**
 *
 * - `ws` — whitespace inside a Svelte tag or block head (`{#…}`, `{:…}`, `{@…}`). Heads
 *   are where tsv hand-rolls its scanning (head splitting, binding/annotation
 *   separation, delimiter finding) rather than delegating to acorn, so they are where a
 *   position rule can be wrong without any well-formed document noticing.
 * - `terminators` — a lone `\r`, `<LS>` or `<PS>` anywhere in the document. These are the
 *   spellings on which the two line classes DISAGREE, and which class a `loc` was counted
 *   under is decided per acorn parse by what Svelte did to the prefix it handed acorn —
 *   three different preparations across five readers, so a document holding one of these
 *   carries two line counts at once. That model is mirror-knowledge about upstream, held
 *   by hand at seven call sites in tsv's Svelte parser, and nothing else grades it: no
 *   fixture can carry a raw `<CR>` (the format path folds it), and real code has none.
 *
 * Both are cheap to reason about: inserting beside existing whitespace is usually
 * parse-preserving, which keeps the oracle-accepts rate high, and where it isn't the
 * variant is simply skipped.
 *
 * **A variant the ORACLE rejects is not a finding.** The comparison buckets a canonical-side
 * throw as `canonical_error` and moves on, so an injection the oracle refuses costs a
 * parse and nothing else. That is what lets the generator be blunt: it does not need
 * to know which positions are legal, only which are worth trying. A variant **tsv** rejects
 * and the oracle accepts is the opposite — an over-rejection, and one of the findings this
 * audit is for — so `subtract_baseline_diffs` keeps it where the base parsed.
 */

import type { SourceFile } from './types.ts';

/**
 * The two families of perturbation, each with its own insert set and its own site rule.
 *
 * They are separate because they test different claims and therefore want different
 * *positions*. `ws` asks whether a hand-rolled scan measures from the token or from the gap
 * — a head-local question. `terminators` asks which line-terminator class a position was
 * counted under, which is a whole-document question: the class of a terminator matters
 * wherever it lands relative to an island, not only inside a head.
 */
export type InjectKind = 'ws' | 'terminators';

/**
 * What gets inserted, per kind, in the order a file's variants are emitted.
 *
 * The `terminators` set is exactly the spellings on which the two line classes DISAGREE —
 * a lone `\r`, `<LS>`, `<PS>`. `\n` and `\r\n` are deliberately absent: both classes count
 * them identically, so injecting one perturbs layout without ever testing the axis. (They
 * are the null controls in `tests/acorn_loc_line_terminators.rs`, where an expectation
 * table can state what "unchanged" means; here a variant that changes nothing is simply
 * dropped by the subtraction pass, so a null control would cost parses and prove nothing.)
 */
const INSERTS: Record<InjectKind, readonly string[]> = {
	ws: [' ', '\n\t'],
	terminators: ['\r', '\u2028', '\u2029']
};

/**
 * Separates a variant's path from its base's. Both the label writer and the subtraction
 * pass key on it, and they must agree — a variant the subtraction cannot recognize is
 * graded against nothing and reports its base's divergences as its own. Kind-agnostic on
 * purpose: the subtraction is about base-vs-variant, never about which family produced the
 * variant, so one marker serves both and a third kind needs no change here.
 */
export const VARIANT_MARKER = '#inj';

/** A head opens with `{` followed by one of these; the scan runs to the matching `}`. */
const HEAD_OPENER_SECONDS = new Set(['#', ':', '@']);

/**
 * Delimiters worth pushing a token away from. `:` is the block-binding annotation
 * separator, `,` the `{#each}` index separator, `=` a snippet parameter default — each
 * a position where a hand-rolled scan decides where one construct ends and the next
 * begins, and so a position where "is the whitespace mine or yours" has an answer that
 * can be wrong.
 */
const DELIMITERS = new Set([':', ',', '=']);

const is_ws = (c: string): boolean => c === ' ' || c === '\t' || c === '\n' || c === '\r';

/**
 * The insert, spelled so a report reader can retype it.
 *
 * `JSON.stringify` is not enough: JSON is a superset of ECMAScript string syntax as of
 * ES2019, so it leaves U+2028 and U+2029 as themselves — and a raw line separator inside a
 * variant's label renders as nothing at all in a terminal, which is exactly the case this
 * family exists to test. Every non-printable-ASCII character therefore gets an explicit
 * `\uXXXX`, so a finding names a perturbation that can be reproduced by hand.
 */
function escape_insert(insert: string): string {
	let out = '';
	for (const c of insert) {
		const code = c.codePointAt(0)!;
		out += code >= 0x20 && code < 0x7f ? c : `\\u${code.toString(16).padStart(4, '0')}`;
	}
	return `"${out}"`;
}

/**
 * The `[start, end)` of every Svelte tag/block head in `source`, by brace matching from
 * each opener.
 *
 * Deliberately approximate — it counts braces without tracking strings or comments, so
 * a head containing `'}'` inside a string ends early. That costs a few injection sites
 * and can never produce a *wrong finding*: a mis-scanned region still yields a variant
 * that both parsers read the same way, or one the oracle rejects. Precision here would
 * mean a second Svelte parser, which is the thing under test.
 */
export function head_regions(source: string): Array<[number, number]> {
	const regions: Array<[number, number]> = [];
	for (let i = 0; i < source.length - 1; i++) {
		// Char compares rather than `HEAD_OPENERS.includes(source.slice(i, i + 2))`: that
		// allocated a two-character string for every byte of every file in the corpus.
		if (source[i] !== '{' || !HEAD_OPENER_SECONDS.has(source[i + 1])) continue;
		let depth = 0;
		let end = -1;
		for (let j = i; j < source.length; j++) {
			if (source[j] === '{') depth++;
			else if (source[j] === '}' && --depth === 0) {
				end = j;
				break;
			}
		}
		if (end > i) {
			regions.push([i + 1, end]);
			i = end;
		}
	}
	return regions;
}

/**
 * Offsets inside `source`'s heads at which inserting whitespace is worth trying: the head's
 * own marker, the start of each existing whitespace run, and each delimiter that has no
 * whitespace before it.
 *
 * The rules cover the ways a position rule goes wrong. Widening an existing run asks "does
 * this construct measure from the token or from the gap?"; inserting before a glued
 * delimiter asks "does it assume the two are adjacent?" — which is precisely the assumption
 * that holds in every document anyone writes by hand.
 *
 * ⚠️ **The marker is its own rule, and it is the sharpest one.** It is not a separator
 * inside the head but the byte that CLASSIFIES the brace, and the two sides of the language
 * answer it differently: Svelte's `tag()` and `read_attribute` run `allow_whitespace()`
 * after the `{`, its `read_sequence` does not. A parser mirroring either reads a byte at a
 * FIXED OFFSET from the brace, which assumes a gap of width zero — and the formatter's brace
 * normalization closes exactly that gap, so the assumption is reachable from the tool's own
 * output. `:` used to reach this by accident, being in `DELIMITERS`; `#` and `@` are not, so
 * the axis was untestable for precisely the two markers a placement rule reads. Making it
 * explicit for all three is what turned three real bugs (`{ @attach}` rejected though prettier
 * formats it; a `{ #x in y}` the printer glued into a form tsv then refused; a static
 * `<script { #a}>` head folding the gap into an attribute name) from invisible into counted.
 */
export function injection_sites(source: string): number[] {
	const sites: number[] = [];
	for (const [start, end] of head_regions(source)) {
		// `head_regions` starts a region AT the marker, and a marker is never whitespace, so
		// this is unconditional — and the inner loop resumes past it rather than at it, or a
		// `{:` would be minted twice (once here, once by the `DELIMITERS` arm).
		sites.push(start);
		for (let i = start + 1; i < end; i++) {
			const prev = source[i - 1] ?? '';
			if (is_ws(source[i])) {
				if (!is_ws(prev)) sites.push(i);
			} else if (DELIMITERS.has(source[i]) && !is_ws(prev)) {
				sites.push(i);
			}
		}
	}
	return sites;
}

/**
 * Offsets at which inserting a **line terminator** is worth trying: the start of every
 * whitespace run in the document, head or not.
 *
 * Document-wide because the line-terminator class is a document-wide fact. Which class a
 * position was counted under depends on where the terminator sits relative to an island —
 * in the prefix acorn measured, in the run acorn *skipped* between its `lineStart` and the
 * position it began lexing, or inside the island itself — and only the first of those is
 * ever in a head. Restricting to heads would test the one region the axis is least about.
 *
 * Whitespace-run starts specifically, because inserting beside existing whitespace is the
 * perturbation least likely to change what parses: in template text it is more text, in a
 * head or a `<script>` it is more token separation, and in a string or template literal both
 * parsers read the same character. So the oracle keeps accepting and the run keeps grading.
 */
export function terminator_sites(source: string): number[] {
	const sites: number[] = [];
	for (let i = 0; i < source.length; i++) {
		if (is_ws(source[i]) && !is_ws(source[i - 1] ?? '')) sites.push(i);
	}
	return sites;
}

/**
 * `count` sites spread evenly across `sites`, or all of them when there are fewer.
 *
 * The cap has to fall somewhere, and taking a **prefix** puts every variant in the first few
 * lines of the file — which for the terminator family is close to worthless, since what it
 * asks is how a terminator interacts with islands *throughout* a document. Striding costs
 * nothing and is deterministic (no `Math.random`, which the repo's harnesses avoid so a
 * finding reproduces from its label alone).
 *
 * ⚠️ **A strided run is a SAMPLE, and the stride divisor is the file's own site count** —
 * so an edit anywhere in a file redraws which of its sites get probed, including in text the
 * edit never touched. That is why a capped run's finding set cannot be ratcheted: measured
 * over `tests/fixtures`, a one-line coverage extension to a single unrelated fixture retired
 * 12 of the terminator family's 194 finding signatures while every underlying bug stood.
 * A census (`limit <= 0`) has no divisor and no such motion. See
 * [audits.md §Wire-Injection Audit](../../../docs/audits.md).
 */
function stride_sample(sites: number[], count: number): number[] {
	if (sites.length <= count) return sites;
	const out: number[] = [];
	for (let k = 0; k < count; k++) out.push(sites[Math.floor((k * sites.length) / count)]);
	return out;
}

/**
 * The injected variants of one file, deterministic in `(content, limit, kinds)` and capped
 * so a site-dense document cannot dominate a run. The budget is split evenly across the
 * requested kinds, so asking for both does not halve either one's reach into the file.
 *
 * **`limit <= 0` is a CENSUS** — every site, no cap. That is the only mode whose finding set
 * is a function of the corpus rather than of the stride, and therefore the only one a run can
 * be graded against over time: a census is monotone under a fixture addition (a new file can
 * only add sites) where a capped run redraws its sample on every edit (see `stride_sample`).
 * Whether a family can afford one is a per-family fact about site density, not a preference —
 * over `tests/fixtures` the `ws` heads hold ~11k sites (a census is seconds) while the
 * document-wide `terminators` sites number ~629k (minutes), which is why only the first is
 * run as one.
 *
 * Each carries a `path` of `<original>#inj:<kind>@<offset>+<escaped insert>` — the label the
 * comparison reports and groups by, so a finding names the exact perturbation that produced
 * it and can be reproduced by hand from the report alone.
 */
export function inject_variants(
	file: SourceFile,
	limit: number,
	kinds: readonly InjectKind[]
): SourceFile[] {
	if (file.language !== 'svelte' || kinds.length === 0) return [];
	const out: SourceFile[] = [];
	// `Infinity` rather than a large sentinel: it flows through the stride, the budget and the
	// per-insert ceiling below as the identity, so the census needs no second code path to
	// drift from the capped one.
	const per_kind = limit <= 0 ? Infinity : Math.max(1, Math.floor(limit / kinds.length));
	for (const kind of kinds) {
		const inserts = INSERTS[kind];
		const all_sites =
			kind === 'ws' ? injection_sites(file.content) : terminator_sites(file.content);
		const budget = out.length + per_kind;
		for (const site of stride_sample(all_sites, Math.ceil(per_kind / inserts.length))) {
			for (const insert of inserts) {
				if (out.length >= budget) break;
				const content = file.content.slice(0, site) + insert + file.content.slice(site);
				out.push({
					...file,
					path: `${file.path}${VARIANT_MARKER}:${kind}@${site}+${escape_insert(insert)}`,
					content,
					bytes: file.bytes + insert.length
				});
			}
		}
	}
	return out;
}

/**
 * Wrap a file stream so each file is followed by its injected variants.
 *
 * The variants ride the same generator rather than a separate pass so that every
 * downstream stage — parse, deep-diff, divergence classification, grouping, the gate —
 * treats a manufactured input exactly like an authored one. A second pass would be a
 * second place for the sanctioned-divergence list to be consulted, and those two lists
 * drifting apart is the failure this shape rules out.
 */
export async function* with_injected_variants(
	files: AsyncGenerator<SourceFile>,
	limit: number,
	kinds: readonly InjectKind[]
): AsyncGenerator<SourceFile> {
	for await (const file of files) {
		yield file;
		for (const variant of inject_variants(file, limit, kinds)) yield variant;
	}
}

/** A result shape narrow enough to subtract over — the fields this pass reads. */
interface SubtractableResult {
	path: string;
	status: string;
	diffs: Array<{ signature: string; documented: string | null }>;
}

/** The statuses that carry no diffs, so subtracting by signature alone cannot grade them. */
const TERMINAL_STATUSES: ReadonlySet<string> = new Set([
	'tsv_error',
	'canonical_error',
	'both_error'
]);

/** The two of those in which tsv is the side that refused. */
const TSV_REFUSED: ReadonlySet<string> = new Set(['tsv_error', 'both_error']);

/**
 * Drop from each injected variant every diff its **base file already had**, and re-grade
 * what remains.
 *
 * Without this the audit is unusable on the one tree worth pointing it at. `tests/fixtures`
 * deliberately contains ~91 `_svelte_divergence` fixtures — documents whose entire purpose
 * is to differ from the canonical parser — so a raw comparison there reports their known
 * divergence once for the base file and once more for every variant derived from it. The
 * injection did not cause those, and an audit that says it did is noise that would be
 * silenced by exclusion lists, which then go stale.
 *
 * The question this audit actually asks is a **delta**: does perturbing whitespace
 * introduce a divergence that was not there before? Subtracting the base's signatures
 * answers exactly that, and it is the same shape the gap and blank injection audits use —
 * grade the injected document against its own un-injected self, never against an absolute
 * expectation.
 *
 * The base files are **controls and are dropped**, not graded. In this mode the run's
 * question is only about manufactured inputs; a base file's own divergence is the fixture
 * tree working as intended (`_svelte_divergence` fixtures exist to diverge) and grading it
 * here would re-report, under an audit about injection, findings that belong to a different
 * gate with a different sanction list.
 *
 * ⚠️ Subtraction is by **signature** (kind + path with indices normalized), not by concrete
 * diff. An injection shifts every offset after it, so the same underlying divergence
 * reports different *values* in the variant than in the base; matching on values would
 * subtract nothing. The cost is that a base already diverging at some signature masks a
 * genuinely new divergence at that same signature in its variants — a blind spot worth
 * naming, and the reason a base file with no divergence at all is the better seed.
 *
 * ⚠️ **One parse failure IS a finding, and it carries no diffs to be subtracted by.** An
 * injection that turns a document tsv accepted into one it **rejects** is an over-rejection —
 * exactly what this audit exists to manufacture — but its row holds `diffs: []`, so a
 * signature-only pass drops it and leaves nothing behind but a number in the dim
 * "parse-fail skipped" line. It is kept, with its `#inj:` label, which is the only thing
 * that makes such a finding reproducible by hand. The base must have PARSED on tsv's side
 * for that to be true: `tests/fixtures` holds ~593 `input_invalid_*` files plus the
 * `tsv_rejects` set, each of which fails identically in every variant derived from it, and
 * a base the oracle also refused is a document tsv never accepted at all.
 *
 * The other two terminal statuses are **not** findings and stay dropped. A variant the
 * ORACLE refuses is the blunt generator working as designed (see this module's header), and
 * one both sides refuse is an injection that simply made the document invalid. Their counts
 * stay raw in the run's own "parse-fail skipped" line, which is where an unfindable
 * manufactured input belongs.
 */
export function subtract_baseline_diffs<T extends SubtractableResult>(results: T[]): T[] {
	const baseline = new Map<string, { signatures: Set<string>; status: string }>();
	for (const r of results) {
		if (r.path.includes(VARIANT_MARKER)) continue;
		baseline.set(r.path, {
			signatures: new Set(r.diffs.map((d) => d.signature)),
			status: r.status
		});
	}
	const kept: T[] = [];
	for (const r of results) {
		const marker = r.path.indexOf(VARIANT_MARKER);
		if (marker === -1) continue; // a control, already consumed into `baseline`
		const base = baseline.get(r.path.slice(0, marker));
		if (r.status === 'tsv_error') {
			// A base that parsed cleanly is absent from `baseline` (exact matches are counted,
			// not stored), so an undefined base is one tsv accepted — which is the condition,
			// since only then is the variant's rejection the injection's doing.
			if (!TSV_REFUSED.has(base?.status ?? 'match')) kept.push(r);
			continue;
		}
		// The oracle refused it, or both sides did: never a finding, and carrying no diffs to
		// be graded by either.
		if (TERMINAL_STATUSES.has(r.status)) continue;
		const diffs = base ? r.diffs.filter((d) => !base.signatures.has(d.signature)) : r.diffs;
		if (diffs.length === 0) continue; // the injection changed nothing — not a finding
		kept.push({
			...r,
			diffs,
			status: diffs.every((d) => d.documented !== null) ? 'documented' : 'undocumented'
		});
	}
	return kept;
}

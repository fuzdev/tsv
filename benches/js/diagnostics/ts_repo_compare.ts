/**
 * `conformance:ts-repo` — tsv's TS parser over the official `microsoft/typescript`
 * compiler's own test corpus (`../typescript/tests/cases`, the WHOLE tree), graded
 * against **tsc's own baselines as the validity oracle**: a
 * `tests/baselines/reference/<name>.errors.txt` carrying a `TS1xxx` code means tsc's
 * grammar rejects, none means tsc accepts. tsc is authoritative because
 * acorn-typescript (tsv's drop-in *shape* target) is itself over-lenient AND
 * over-strict against the real compiler, so it is only a sub-label here.
 *
 * The model — every bucket, what each ledger means, the reading rules, and the
 * triage workflow — is `docs/conformance_tsc.md`. This header is the operator's
 * card; the doc is the reference.
 *
 * Per parse unit (a single-file case, or one `@filename` unit of a multi-file test
 * whose baselines carry no grammar error at all):
 *
 *   tsv accepts, tsc valid        → accept_parity
 *   tsv accepts, tsc invalid      → over_acceptance, split by tsc's LIVE PARSER:
 *                                     `parser`  tsc's parser refuses too (a question for the grammar line)
 *                                     `checker` tsc's parser accepts, a checker-side grammar check refuses
 *                                               (the deferred early-error posture, by design)
 *   tsv rejects, tsc invalid      → reject_parity
 *   tsv rejects, tsc valid        → an over-rejection, attributed in order:
 *     parses at Script goal, and tsc reads the file as a script   → script_goal        (a goal artifact, not a gap)
 *     tsc's own parser rejects the raw bytes                     → tsc_parser_rejects (encoding / harness artifacts)
 *     acorn accepts   → TS_REPO_SANCTIONS / KNOWN_GAPS / **UNTRACKED — GATES**
 *     acorn rejects   → BEYOND_ACORN_SANCTIONS / BEYOND_ACORN_KNOWN_GAPS / **UNTRACKED — GATES**
 *
 * Sanction = keep deliberately (a production tsv's grammar refuses that tsc's parser
 * only recovers through, a tsc-only extension, a declaration-file rule); known gap =
 * fix eventually (a pending reject→defer flip, or a genuine gap acorn shares). Every
 * ledger is freshness-checked on a full-corpus run: an entry matching nothing FAILS,
 * so a fixed gap must leave the ledger the day it is fixed. The exact pins in
 * `lib/gate_counts.ts` (`TS_REPO_PINS`) hold the counts the ledgers cannot: a
 * checkout pull, a widening, or a collapsed oracle must be re-pinned deliberately.
 *
 * Scope: `.tsx` is skipped (JSX grammar, out of scope), `.d.ts` is graded (see
 * `DECLARATIONS`), a multi-file test with a grammar error anywhere is skipped as
 * `multi_file_tainted` (the baseline cannot say which unit), and a unit that is not
 * TypeScript (`.js`, `package.json`, …) is counted, never graded. Discovery, the
 * unit split, and the baseline rules are SHARED with the tsc-corpus harvest
 * (`lib/ts_repo.ts`), so the gate and the bench corpus cannot drift on what a parse
 * unit IS. Compiler-directive comments (`// @target`) are inert to a parser, so
 * nothing is stripped; the one shape that changes (`#!` under a directive) lands in
 * `tsc_parser_rejects`, since tsc's parser sees the same bytes.
 *
 * SEPARATE from the acorn-typescript-suite gate (`ts_fixtures_compare.ts`): that
 * gate's oracle is the live acorn parser over acorn's OWN `test/` suite; this one's
 * is tsc's baselines over the compiler corpus. Their ledgers are kept apart.
 *
 * A leg of the blocking `conformance` aggregate (publish Step 3b). Summary to
 * stderr, full JSON to stdout with `--json`. `run_ts_repo_compare` is the importable
 * entry the `conformance.ts` single-process driver calls; the CLI guard at the
 * bottom feeds it `Deno.args`. Failure semantics are process-level (`Deno.exit(1)`).
 *
 * Setup posture: strict — a missing `../typescript` checkout, a PARTIAL checkout
 * (baselines or the corpus subtree missing), or an empty scan all FAIL rather than
 * green-skipping (the baselines are the oracle; publish Step 3b's preflight probe is
 * the tolerance point for machines without the checkout).
 *
 * Run (from the repo root):
 *   deno task conformance:ts-repo            # builds corpus FFI, then runs
 *   deno task conformance:ts-repo:run        # skip rebuild (freshness-guarded)
 *   deno task conformance:ts-repo:run --json 2>/dev/null > report.json
 *   deno task conformance:ts-repo:run -v     # every ledgered / auto-bucketed entry
 *   deno task conformance:ts-repo:run ../typescript/tests/cases/conformance/parser/ecmascript6
 */

import { readdir, readFile, stat } from 'node:fs/promises';
import { basename, join } from 'node:path';

import { init_compare_implementations } from '../lib/compare_cli.ts';
import { TS_REPO_PINS } from '../lib/gate_counts.ts';
import { type KnownGap, TS_REPO_SANCTIONS } from '../lib/parse_sanctions.ts';
import { load_typescript, tsc_parse } from '../lib/tsc.ts';
import {
	baseline_test_key,
	discover_ts_cases,
	empty_ts_case_skips,
	grammar_error_codes,
	is_multi_file_test,
	split_test_units,
	test_unit_language,
	TS_BASELINE_DIR,
	TS_REPO
} from '../lib/ts_repo.ts';

/**
 * The WHOLE corpus, not the `conformance/parser` subtree.
 *
 * The parser tests are not where parser bugs live: the checker and emitter trees are
 * syntactically ordinary TS written to trip semantic rules, and a parse gap surfacing
 * in shallow test code is likelier reachable in real code than one buried in the
 * parser torture suite; a root narrower than the whole tree has sat green over real
 * over-rejections in exactly those trees. The extra trees (`fourslash`, `projects`,
 * `transpile`, `unittests`) add no known gaps and cost about two seconds warm, so
 * scanning everything is cheaper than keeping a second root right.
 *
 * The tsc-corpus **bench harvest** (`harvest_ts_repo.ts`) deliberately keeps the
 * narrower `conformance` + `compiler` pair: it builds a *corpus of valid TS* to
 * measure coverage on, where the extra trees add fixture noise, while this gate hunts
 * *over-rejections*, where a superset can only help. `lib/ts_repo.ts` is what the two
 * must agree on — the parse-unit rule and the baseline rules — not the scope.
 */
const DEFAULT_ROOT = `${TS_REPO}/tests/cases`;

/**
 * `.d.ts` cases are GRADED here: this gate hunts over-rejections, and a declaration
 * file is ordinary TypeScript to tsv — no declaration mode exists, `tsv format`
 * discovers a `.d.ts` like any other `.ts`, so a parse gap in one ships. The bench
 * harvest skips them for a reason of its OWN (its consumers get content with no
 * path — `harvest_ts_repo.ts` `DECLARATIONS`).
 *
 * `baseline_test_key` already keys `<name>.d.ts` to its `<name>.d.errors.txt`
 * baseline, so the oracle reads them unchanged. Two soft spots come with them, both
 * reported rather than gated: a construct valid ONLY under tsc's filename-keyed
 * declaration-file rule (`await` as a name in a `.d.ts` module) is a
 * `declaration_file` sanction, and under `tests/cases/projects/` a DIFFERENT harness
 * compiles whole scenarios and baselines only their declared inputs, so a file no
 * scenario names has no baseline and reads as tsc-valid — those land in
 * `tsc_parser_rejects` when tsc's own parser refuses them, as the one stale TS-1.x
 * emit artifact there does.
 */
const DECLARATIONS = 'include' as const;

/**
 * Whether `root` is the default corpus, for the full-corpus-only hygiene block.
 *
 * Normalized rather than compared with `===`: the default is a path a person
 * plausibly types by hand, and a trailing slash (`…/tests/cases/`) would silently
 * skip BOTH the ledger freshness checks and the `TS_REPO_PINS` check — a run that
 * looks like the full gate but grades strictly less.
 */
function is_default_root(root: string): boolean {
	const strip = (p: string) => p.replace(/\/+$/, '');
	return strip(root) === strip(DEFAULT_ROOT);
}

/**
 * tsv parse gaps confirmed against BOTH oracles (tsc-valid AND acorn-accepts),
 * tracked so the tool is green at baseline and a NEW gap surfaces. Must only
 * SHRINK: delete an entry when its gap is fixed. Kept SEPARATE from
 * `ts_fixtures_compare.ts` KNOWN_GAPS (different corpus + oracle). `pattern` is a
 * substring of the ledger key — `<path>`, or `<path>::<unit>` for a multi-file
 * unit — spelled `<basename>.ts` to avoid numeric-suffix collisions
 * (`…Declaration1` vs `…Declaration11`), and `/<basename>.ts` where one basename is
 * a suffix of another (`ParameterList5.ts` is inside `parserParameterList5.ts`).
 * Every ledger below shares the rule.
 */
const KNOWN_GAPS: KnownGap[] = [
	// Empty: a new tsc-accepted, acorn-accepted over-rejection surfaces as an
	// untracked `gap` (exits 1). Deliberate non-support goes in TS_REPO_SANCTIONS.
];

/**
 * Why an over-rejection acorn ALSO makes is kept. The category is the argument,
 * stated once here so an entry can name it in a word:
 *
 * - `grammar` — a rule that lives in a PRODUCTION (ecma262's, the TS grammar's, or a
 *   finished proposal's): a spelling the grammar simply has no production for. tsc's
 *   parser reads past it for error recovery and its checker reports; the spec governs
 *   and tsv refuses at parse (docs/conformance_tsc.md §The reject-vs-defer line).
 * - `tsc_recovery` — a TS-only position where tsc's parser is deliberately LOOSER than
 *   TypeScript's published grammar so the checker can say something useful (an
 *   expression where a type reference belongs, any property name where an
 *   identifier belongs). No other parser follows; matching it would mean tracking
 *   tsc's recovery strategy rather than its grammar.
 * - `tsc_extension` — syntax tsc parses that is not TypeScript at all: JSDoc type
 *   forms inside a `.ts` file (parsed so TS8020 can be reported), U+0085 as a line
 *   break (ECMAScript's WhiteSpace and LineTerminator productions exclude it).
 * - `declaration_file` — valid ONLY under tsc's filename-keyed `.d.ts` rule; tsv
 *   has no declaration mode (the harvest's `DECLARATIONS` argument).
 */
interface BeyondAcornSanction {
	pattern: string;
	category: 'grammar' | 'tsc_recovery' | 'tsc_extension' | 'declaration_file';
	reason: string;
}

/**
 * Why an over-rejection acorn ALSO makes is a bug — the two kinds a fix can close:
 *
 * - `reject_flip` — a pending reject→defer flip: tsc's parser accepts and its checker
 *   reports, so the reject-vs-defer line (docs/conformance_tsc.md) puts it on the
 *   defer side. The flip closes the entry.
 * - `beyond_acorn_gap` — a genuine gap acorn-typescript shares: tsc, prettier's
 *   `typescript` parser (and usually babel) accept, the grammar has the production,
 *   and tsv should diverge from acorn toward it. A fix here is a `_svelte_divergence`
 *   fixture (canonical rejects, tsv parses).
 */
interface BeyondAcornKnownGap extends KnownGap {
	category: 'reject_flip' | 'beyond_acorn_gap';
}

/**
 * Over-rejections where tsc's baselines say valid, acorn rejects, and tsv KEEPS
 * rejecting — the `beyond_acorn` bucket's sanctions. Categories: {@link
 * BeyondAcornSanction}. Each entry is one reviewed verdict; a shape here is never a
 * bug to fix, and a shape that becomes one moves to `BEYOND_ACORN_KNOWN_GAPS`.
 */
const BEYOND_ACORN_SANCTIONS: BeyondAcornSanction[] = [
	// --- grammar ---------------------------------------------------------------
	// A no-declaration for-in/of head is a LeftHandSideExpression position, NOT an
	// assignment context: a call / `new` / `this` / update expression / literal /
	// bare cast there stays a parse error (conformance_svelte.md §TypeScript
	// Corrections, the nonsimple_target + cast_target entries). tsc's parser takes
	// any expression and reports TS2405/TS2406 from the checker.
	...['parserForStatement6.ts', 'parserForStatement7.ts', 'parserForStatement8.ts'].map(
		(pattern) => ({
			pattern,
			category: 'grammar' as const,
			reason: 'for-in head with a call / `new` / `this` target — not an assignment context'
		})
	),
	{
		pattern: 'for-of3.ts',
		category: 'grammar',
		reason: '`for (v++ of …)` — an UpdateExpression is not a LeftHandSideExpression'
	},
	{
		pattern: 'ES5For-of12.ts',
		category: 'grammar',
		reason: '`for ([""] of …)` — a string literal is not a DestructuringAssignmentTarget'
	},
	{
		pattern: 'referenceSatisfiesExpression.ts',
		category: 'grammar',
		reason:
			'`for ((g satisfies T) of …)` — a bare cast target in a for-of head (the assignment / pattern spellings in the same file parse)'
	},
	{
		pattern: 'elidedEmbeddedStatementsReplacedWithSemicolon.ts',
		category: 'grammar',
		reason: '`with` statement — no strict-mode production (CLAUDE.md §Strict Mode Only)'
	},
	{
		pattern: 'withStatementInternalComments.ts',
		category: 'grammar',
		reason: '`with` statement — no strict-mode production (CLAUDE.md §Strict Mode Only)'
	},
	{
		pattern: 'topLevelVarHoistingCommonJS.ts',
		category: 'grammar',
		reason: '`with` statement — no strict-mode production (CLAUDE.md §Strict Mode Only)'
	},
	// PrivateIdentifier has exactly three productions: a ClassElementName, a member
	// access, and the left operand of `in`. tsc's parser reads it as any property
	// name and reports TS18016-family errors from the checker.
	...[
		'privateNameBadDeclaration.ts',
		'privateNameInObjectLiteral-1.ts',
		'privateNameInObjectLiteral-2.ts',
		'privateNameInObjectLiteral-3.ts'
	].map((pattern) => ({
		pattern,
		category: 'grammar' as const,
		reason: 'a `#name` as an object-literal member — PrivateIdentifier has no such production'
	})),
	{
		pattern: 'privateNameAndPropertySignature.ts',
		category: 'grammar',
		reason: 'a `#name` property signature in an interface — PropertySignature takes a PropertyName'
	},
	// BindingRestElement / BindingRestProperty is the production's LAST element, and
	// carries a BindingIdentifier, not a property name. tsc recovers and reports
	// TS2462 from the checker.
	...['objectRestNegative.ts', 'objectRestPropertyMustBeLast.ts', 'restElementMustBeLast.ts'].map(
		(pattern) => ({
			pattern,
			category: 'grammar' as const,
			reason: 'a rest element that is not last — the production places it last'
		})
	),
	{
		pattern: 'objectBindingPattern_restElementWithPropertyName.ts',
		category: 'grammar',
		reason: '`{ ...a: b }` — BindingRestProperty is `... BindingIdentifier`, no property name'
	},
	{
		pattern: 'misspelledNewMetaProperty.ts',
		category: 'grammar',
		reason: '`new.targt` — MetaProperty is `new . target` exactly'
	},
	{
		pattern: 'importMetaPropertyInvalidInCall.ts',
		category: 'grammar',
		reason:
			'`import.foo` — MetaProperty is `import . meta` exactly (and the `import.defer(…)` call form)'
	},
	{
		pattern: 'importAssertionNonstring.ts',
		category: 'grammar',
		reason: '`with { field: 0 }` — a WithEntries value is a StringLiteral by production'
	},
	{
		pattern: 'importAttributes6.ts',
		category: 'grammar',
		reason: '`with { field: 0 }` — a WithEntries value is a StringLiteral by production'
	},
	// `import defer` admits only the namespace form (`import defer * as ns from`);
	// a default binding or a named clause behind `defer` has no production.
	...[
		'importDeferFromInvalid.ts',
		'importDeferInvalidDefault.ts',
		'importDeferInvalidNamed.ts'
	].map((pattern) => ({
		pattern,
		category: 'grammar' as const,
		reason:
			'a default or named clause behind `import defer` — only the namespace form has a production'
	})),
	// --- tsc_recovery ----------------------------------------------------------
	// An `implements` / interface `extends` heritage element is a type reference
	// (EntityName + type arguments) by the TS grammar; tsc's parser reads a
	// LeftHandSideExpression there for recovery and reports TS2499 / TS2507 from the
	// checker. ES `class extends` takes any LeftHandSideExpression, and tsv accepts
	// the `extends A?.B` line in the same file. The reserved-word heritage-name
	// family was settled the same way (the simpler-implementation tiebreaker,
	// docs/conformance_tsc.md §The reject-vs-defer line).
	...[
		'classExtendingOptionalChain.ts',
		'interfaceExtendingOptionalChain.ts',
		'interfaceMayNotBeExtendedWitACall.ts',
		'declarationEmitInterfaceWithNonEntityNameExpressionHeritage.ts'
	].map((pattern) => ({
		pattern,
		category: 'tsc_recovery' as const,
		reason: 'a heritage element that is not a type reference (`implements A?.B`, `extends a()`)'
	})),
	// An EnumMember's name is an IdentifierName or a string literal; tsc's parser
	// reads any PropertyName (numeric, bigint, computed, private) for recovery and
	// reports TS2452 from the checker. No other parser follows.
	...[
		'enumIdentifierLiterals.ts',
		'enumWithBigint.ts',
		'literalsInComputedProperties1.ts',
		'privateNameEnum.ts',
		'parserEnum7.ts'
	].map((pattern) => ({
		pattern,
		category: 'tsc_recovery' as const,
		reason: 'an enum member named by a number / bigint / computed key / `#name`'
	})),
	// --- tsc_extension ---------------------------------------------------------
	// JSDoc type syntax inside a `.ts` file — `?T`, `T?`, `!T`, `T!`, `<?>`, `a.<T>`
	// — which tsc parses so it can report TS8020 from the checker.
	...[
		'parseInvalidNonNullableTypes.ts',
		'parseInvalidNullableTypes.ts',
		'decoratorMetadata-jsdoc.ts',
		'expressionWithJSDocTypeArguments.ts',
		'jsdocDisallowedInTypescript.ts',
		'namedTupleMembersErrors.ts'
	].map((pattern) => ({
		pattern,
		category: 'tsc_extension' as const,
		reason: 'JSDoc type syntax in a `.ts` file — not TypeScript grammar'
	})),
	{
		pattern: 'fileWithNextLine2.ts',
		category: 'tsc_extension',
		reason: 'U+0085 NEXT LINE as whitespace — outside ECMAScript’s WhiteSpace and LineTerminator'
	},
	{
		pattern: 'sourceMap-LineBreaks.ts',
		category: 'tsc_extension',
		reason: 'U+0085 NEXT LINE as a line break — outside ECMAScript’s LineTerminator'
	},
	// --- declaration_file ------------------------------------------------------
	{
		pattern: 'topLevelAwait.3.ts::index.d.ts',
		category: 'declaration_file',
		reason:
			'`declare const await` in a `.d.ts` module — tsc’s filename-keyed declaration-file rule; tsv has no declaration mode'
	}
];

/**
 * Over-rejections where tsc's baselines say valid, acorn rejects, and tsv is WRONG —
 * to be fixed, so this list must only SHRINK. Categories: {@link BeyondAcornKnownGap}.
 */
const BEYOND_ACORN_KNOWN_GAPS: BeyondAcornKnownGap[] = [
	// --- reject_flip -----------------------------------------------------------
	...[
		'/ParameterList13.ts',
		'defaultArgsInOverloads.ts',
		'/parserParameterList13.ts',
		'callSignaturesWithAccessibilityModifiersOnParameters.ts',
		'callSignaturesWithParameterInitializers.ts',
		'constructSignatureWithAccessibilityModifiersOnParameters.ts',
		'constructSignatureWithAccessibilityModifiersOnParameters2.ts'
	].map((pattern) => ({
		pattern,
		category: 'reject_flip' as const,
		reason:
			'a parameter default / parameter property in an overload or call / construct signature — TS2371 / TS2369 are checker-raised (`check_signature_params` flip)'
	})),
	...[
		'/ParameterList5.ts',
		'/ParameterList6.ts',
		'/parserParameterList5.ts',
		'/parserParameterList6.ts'
	].map((pattern) => ({
		pattern,
		category: 'reject_flip' as const,
		reason:
			'a parameter-property modifier in a function TYPE’s parameters (`(public B) => C`) — TS2369 checker-raised; grades with the signature-parameter flip'
	})),
	{
		pattern: 'defaultValueInFunctionTypes.ts',
		category: 'reject_flip',
		reason:
			'a parameter default in a function TYPE (`(a = 1) => void`) — TS2371 checker-raised; grades with the signature-parameter flip'
	},
	{
		pattern: 'privateNameConstructorReserved.ts',
		category: 'reject_flip',
		reason: 'a `#constructor` class element — an ES early error, TS18012 checker-raised'
	},
	{
		pattern: 'propertyNamedConstructor.ts',
		category: 'reject_flip',
		reason: 'a field named `constructor` — an ES early error'
	},
	{
		pattern: 'thisTypeInAccessors.ts',
		category: 'reject_flip',
		reason: 'an accessor `this` parameter — TS2784 checker-raised'
	},
	// --- beyond_acorn_gap ------------------------------------------------------
	{
		pattern: 'importDefaultNamedType2.ts',
		category: 'beyond_acorn_gap',
		reason:
			'`import type from from "./a"` — a type-only default import whose binding is named `from`'
	},
	{
		pattern: 'importDefaultNamedType3.ts',
		category: 'beyond_acorn_gap',
		reason:
			'`import type from from "./a"` — a type-only default import whose binding is named `from`'
	},
	{
		pattern: 'arbitraryModuleNamespaceIdentifiers_module.ts',
		category: 'beyond_acorn_gap',
		reason:
			'`import { type "<A>" as typeA }` — a string-named specifier behind the `type` modifier (the unmodified `{ "<X>" as x }` in the same file parses)'
	},
	{
		pattern: 'usingDeclarationsInFor.ts',
		category: 'beyond_acorn_gap',
		reason:
			'`for (using x = …;;)` — explicit resource management admits a `using` LexicalDeclaration in a C-style for head (only for-in is excluded); tsc, babel and prettier accept'
	},
	{
		pattern: 'awaitUsingDeclarationsInFor.ts',
		category: 'beyond_acorn_gap',
		reason: '`for (await using x = …;;)` — the same for-head production as `using`'
	}
];

function first_line(e: unknown): string {
	return String(e instanceof Error ? e.message : e).split('\n')[0];
}

/** Where a graded parse unit came from — a single-file case, or one unit of a multi-file test. */
interface Located {
	path: string;
	/** The `@filename` unit's name; absent for a single-file case. */
	unit?: string;
}

/** A unit's ledger key, and its display name: `<path>`, or `<path>::<unit>`. */
function locate(at: Located): string {
	return at.unit === undefined ? at.path : `${at.path}::${at.unit}`;
}

/** An over-rejection, carrying the ledger entry it matched when one did. */
interface Gap extends Located {
	tsv_error: string;
	category?: string;
	reason?: string;
}

interface OverAcceptance extends Located {
	/** The `TS1xxx` codes the file's baselines carry (every variant). */
	baseline_codes: string[];
	/** tsc's live parser diagnostics on the file (`parser` bucket only). */
	tsc_parser_codes?: number[];
}

/** What every ledger entry shares; `category` is the sanction / gap kind where the ledger has one. */
interface LedgerEntry {
	pattern: string;
	category?: string;
	reason: string;
}

/**
 * A ledger under freshness tracking: `used` collects the patterns that matched
 * something this run, so the full-corpus hygiene block can name the stale ones.
 */
interface Ledger<E extends LedgerEntry> {
	name: string;
	entries: readonly E[];
	used: Set<string>;
}

function ledger<E extends LedgerEntry>(name: string, entries: readonly E[]): Ledger<E> {
	return { name, entries, used: new Set() };
}

/** The first entry whose pattern `key` contains, recorded as used; `undefined` = untracked. */
function match_ledger<E extends LedgerEntry>(ledger: Ledger<E>, key: string): E | undefined {
	const entry = ledger.entries.find((e) => key.includes(e.pattern));
	if (entry) ledger.used.add(entry.pattern);
	return entry;
}

/** A gap record from its location, tsv's error, and the entry it matched (none = untracked). */
function gap(at: Located, tsv_error: string, entry?: LedgerEntry): Gap {
	return entry
		? { ...at, tsv_error, category: entry.category, reason: entry.reason }
		: { ...at, tsv_error };
}

/** The tool's entry — see the module docstring for buckets, posture, and CLI use. */
export async function run_ts_repo_compare(argv: string[] = Deno.args): Promise<void> {
	const flags = new Set(argv.filter((a) => a.startsWith('-')));
	const json_mode = flags.has('--json');
	const verbose = flags.has('--verbose') || flags.has('-v');
	const root = argv.find((a) => !a.startsWith('-')) ?? DEFAULT_ROOT;

	// A run that can't grade anything is a failure, not a pass — checked before
	// any other work so the error is this message, not a raw ENOENT stack trace.
	// The tolerance point for machines without the checkout is publish Step 3b's
	// preflight probe, which skips the whole aggregate with a warning.
	try {
		await stat(TS_REPO);
	} catch {
		console.error(
			`FAIL: ${TS_REPO} checkout not found — nothing can be graded. ` +
				`Clone microsoft/TypeScript at ${TS_REPO}.`
		);
		Deno.exit(1);
	}

	// Index of every `*.errors.txt` baseline, keyed by its **un-suffixed** test name
	// (`baseline_test_key`, shared with the tsc-corpus harvest — most corpus files are
	// compiled under multiple settings and so carry only per-variant baselines).
	// Index once so a lookup gathers every variant.
	const errors_baselines_by_test = new Map<string, string[]>();
	let baseline_names: string[];
	try {
		baseline_names = await readdir(TS_BASELINE_DIR);
	} catch (e) {
		console.error(
			`FAIL: cannot read ${TS_BASELINE_DIR} (${e instanceof Error ? e.message : e}) — ` +
				`the ${TS_REPO} checkout exists but its baselines are missing (partial/sparse checkout?). ` +
				`The baselines ARE the oracle, so this run cannot grade anything.`
		);
		Deno.exit(1);
	}
	for (const name of baseline_names) {
		if (!name.endsWith('.errors.txt')) continue;
		const key = baseline_test_key(name);
		(errors_baselines_by_test.get(key) ?? errors_baselines_by_test.set(key, []).get(key)!).push(
			name
		);
	}

	/**
	 * tsc's grammar verdict for a test, read from its baseline(s) (all target/module
	 * variants): the distinct `TS1xxx` codes, empty when tsc accepts. Grammar errors
	 * are target-independent, so a code in ANY variant counts.
	 */
	async function baseline_grammar_codes(file_path: string): Promise<string[]> {
		const base = basename(file_path).replace(/\.ts$/, '');
		const codes = new Set<string>();
		for (const name of errors_baselines_by_test.get(base) ?? []) {
			for (const code of grammar_error_codes(await readFile(join(TS_BASELINE_DIR, name), 'utf8'))) {
				codes.add(code);
			}
		}
		return [...codes].sort();
	}

	const { canonical, native } = await init_compare_implementations();
	const ts = await load_typescript();

	/** tsc's live parser over the same bytes: its diagnostics codes, and whether it read a module. */
	function tsc_parser_verdict(
		file_name: string,
		content: string
	): { codes: number[]; is_module: boolean } {
		const { source_file, diagnostics } = tsc_parse(ts, file_name, content);
		if (!diagnostics) {
			console.error(
				'FAIL: tsc SourceFile has no `parseDiagnostics` — the internal field this gate splits ' +
					'over-acceptance with is gone (upstream rename?). Refusing to grade every file as accepted.'
			);
			Deno.exit(1);
		}
		return {
			codes: [...new Set(diagnostics.map((d) => d.code))],
			is_module:
				(source_file as { externalModuleIndicator?: unknown }).externalModuleIndicator !== undefined
		};
	}

	// The four ledgers, each with this run's freshness set (see the stale check at the end).
	const ledgers = {
		sanctions: ledger('TS_REPO_SANCTIONS', TS_REPO_SANCTIONS),
		known: ledger('KNOWN_GAPS', KNOWN_GAPS),
		beyond_sanctions: ledger('BEYOND_ACORN_SANCTIONS', BEYOND_ACORN_SANCTIONS),
		beyond_known: ledger('BEYOND_ACORN_KNOWN_GAPS', BEYOND_ACORN_KNOWN_GAPS)
	};

	const buckets = {
		accept_parity: 0,
		reject_parity: 0,
		over_acceptance_parser: [] as OverAcceptance[],
		over_acceptance_checker: [] as OverAcceptance[],
		script_goal: [] as Gap[],
		tsc_parser_rejects: [] as (Gap & { tsc_parser_codes: number[] })[],
		sanctioned: [] as Gap[],
		gap_known: [] as Gap[],
		gap_unexpected: [] as Gap[],
		beyond_acorn_sanctioned: [] as Gap[],
		beyond_acorn_known: [] as Gap[],
		beyond_acorn_unexpected: [] as Gap[]
	};
	/** File counts per baseline `TS1xxx` code over BOTH over-acceptance buckets. */
	const over_acceptance_codes = new Map<string, number>();
	// `.tsx` is filtered (and counted) inside discovery, which admits `.d.ts` here
	// (`DECLARATIONS`); the rest are content-level decisions the loop makes.
	const discovery_skips = empty_ts_case_skips();
	const skipped = {
		multi_file_tainted: 0,
		unreadable: 0,
		units_tsx: 0,
		units_js: 0,
		units_other: 0
	};
	/** Single-file cases graded (every bucket but the unit counters) — the `scanned` pin. */
	let scanned = 0;
	const multi_file = { tests: 0, graded: 0, units_scanned: 0, units_accept_parity: 0 };
	// Reported, not skipped: `.d.ts` is graded here (`DECLARATIONS`), and a run that
	// shows only its skips can't show that. Counted post-content so it matches the
	// graded population, not the discovered one.
	let declarations_graded = 0;

	/** One parse unit through the whole ladder (the module docstring's table). */
	function grade(at: Located, content: string, baseline_codes: string[]): void {
		// tsc keys its declaration-file rules on the name, so a unit is parsed under its own.
		const file_name = at.unit ?? at.path;
		const tsc_valid = baseline_codes.length === 0;
		let tsv_err: string | null = null;
		try {
			native.parse_internal(content, 'typescript');
		} catch (e) {
			tsv_err = first_line(e);
		}

		if (!tsv_err) {
			if (tsc_valid) {
				if (at.unit === undefined) buckets.accept_parity++;
				else multi_file.units_accept_parity++;
				return;
			}
			// tsv accepts what tsc's baseline refuses. Ask tsc's PARSER whether it is a
			// parser rejection or a checker-side grammar check — the code range cannot say.
			for (const code of baseline_codes) {
				over_acceptance_codes.set(code, (over_acceptance_codes.get(code) ?? 0) + 1);
			}
			const verdict = tsc_parser_verdict(file_name, content);
			if (verdict.codes.length > 0) {
				buckets.over_acceptance_parser.push({
					...at,
					baseline_codes,
					tsc_parser_codes: verdict.codes
				});
			} else {
				buckets.over_acceptance_checker.push({ ...at, baseline_codes });
			}
			return;
		}

		// tsv rejects.
		if (!tsc_valid) {
			buckets.reject_parity++;
			return;
		}
		// tsv rejects, tsc accepts → an over-rejection. Attribute it, cheapest first.
		const verdict = tsc_parser_verdict(file_name, content);
		if (!verdict.is_module) {
			// tsc reads the file as a script (no import/export), and tsv's Module goal is
			// what refused it — `await` as a name, a non-async `await` call. Not a gap when
			// the Script goal takes it: tsv accepts the file at the goal tsc assigns.
			let script_ok = true;
			try {
				native.parse_internal(content, 'typescript', 'script');
			} catch {
				script_ok = false;
			}
			if (script_ok) {
				buckets.script_goal.push(gap(at, tsv_err));
				return;
			}
		}
		if (verdict.codes.length > 0) {
			// tsc's OWN parser refuses these bytes too: a UTF-16 file read as UTF-8
			// (TS1490), a `#!` under a directive line, a stale emit artifact. The baseline
			// is silent only because the harness compiled something else.
			buckets.tsc_parser_rejects.push({ ...gap(at, tsv_err), tsc_parser_codes: verdict.codes });
			return;
		}
		// Sub-label by acorn's verdict, then the ledgers: sanctioned, known, or untracked.
		const key = locate(at);
		let acorn_ok = true;
		try {
			canonical.parse(content, 'typescript');
		} catch {
			acorn_ok = false;
		}
		if (acorn_ok) {
			const sanction = match_ledger(ledgers.sanctions, key);
			if (sanction) {
				buckets.sanctioned.push(gap(at, tsv_err, sanction));
				return;
			}
			const known = match_ledger(ledgers.known, key);
			(known ? buckets.gap_known : buckets.gap_unexpected).push(gap(at, tsv_err, known));
			return;
		}
		const sanction = match_ledger(ledgers.beyond_sanctions, key);
		if (sanction) {
			buckets.beyond_acorn_sanctioned.push(gap(at, tsv_err, sanction));
			return;
		}
		const known = match_ledger(ledgers.beyond_known, key);
		(known ? buckets.beyond_acorn_known : buckets.beyond_acorn_unexpected).push(
			gap(at, tsv_err, known)
		);
	}

	for await (const path of discover_ts_cases(root, discovery_skips, DECLARATIONS)) {
		let content: string;
		try {
			content = await readFile(path, 'utf8');
		} catch {
			skipped.unreadable++;
			continue;
		}
		const baseline_codes = await baseline_grammar_codes(path);
		if (is_multi_file_test(content)) {
			multi_file.tests++;
			// A grammar error somewhere in the test cannot be attributed to one unit, so
			// the whole test is ungraded: the units are graded as tsc-valid only when
			// the baselines are clean, which is what makes each one a positive claim.
			if (baseline_codes.length > 0) {
				skipped.multi_file_tainted++;
				continue;
			}
			multi_file.graded++;
			for (const unit of split_test_units(content)) {
				const language = test_unit_language(unit.name);
				if (language === 'tsx') skipped.units_tsx++;
				else if (language === 'js') skipped.units_js++;
				else if (language === 'other') skipped.units_other++;
				if (language !== 'ts') continue;
				multi_file.units_scanned++;
				grade({ path, unit: unit.name }, unit.content, []);
			}
			continue;
		}
		if (path.endsWith('.d.ts')) declarations_graded++;
		scanned++;
		grade({ path }, content, baseline_codes);
	}

	canonical.dispose();
	native.dispose();

	// --- Report -----------------------------------------------------------------

	const by_category = (gaps: Gap[]): Map<string, number> => {
		const counts = new Map<string, number>();
		for (const g of gaps) counts.set(g.category!, (counts.get(g.category!) ?? 0) + 1);
		return counts;
	};
	const category_line = (counts: Map<string, number>): string =>
		[...counts.entries()].map(([c, n]) => `${c}=${n}`).join(', ') || 'none';

	// The checkout exists (guarded above), so an empty scan means a wrong subtree
	// path or a gutted corpus — a broken invocation, not a pass.
	if (scanned === 0) {
		console.error(
			`FAIL: 0 single-file .ts scanned under ${root} — wrong path? Nothing was graded.`
		);
		Deno.exit(1);
	}

	const unexpected_total = buckets.gap_unexpected.length + buckets.beyond_acorn_unexpected.length;

	console.error(`\nTypeScript-repo parse-conformance gate — root: ${root}`);
	console.error(`  oracle: tsc baselines (${TS_BASELINE_DIR}), tsc's parser for the splits`);
	console.error(
		`  scanned: ${scanned} single-file .ts, incl. ${declarations_graded} .d.ts  ` +
			`(skipped ${discovery_skips.tsx} .tsx, ${skipped.unreadable} unreadable)`
	);
	console.error(
		`  multi-file: ${multi_file.tests} tests, ${multi_file.graded} graded → ${multi_file.units_scanned} .ts units  ` +
			`(skipped ${skipped.multi_file_tainted} tainted tests; ${skipped.units_tsx} .tsx, ` +
			`${skipped.units_js} .js, ${skipped.units_other} other units)\n`
	);
	console.error(
		`  parity accept (tsv ok, tsc valid):      ${buckets.accept_parity} files + ${multi_file.units_accept_parity} units`
	);
	console.error(`  parity reject (tsv + tsc both reject):  ${buckets.reject_parity}`);
	console.error(
		`  over-acceptance, tsc PARSER rejects:    ${buckets.over_acceptance_parser.length}  (a question for the grammar line; pinned)`
	);
	console.error(
		`  over-acceptance, checker-side only:     ${buckets.over_acceptance_checker.length}  (deferred early errors by design; pinned)`
	);
	console.error(
		`  script-goal (tsc reads a script):       ${buckets.script_goal.length}  (goal artifact, not a gap)`
	);
	console.error(
		`  tsc-parser-rejects (raw bytes):         ${buckets.tsc_parser_rejects.length}  (encoding / harness artifacts)`
	);
	console.error(
		`  sanctioned (acorn ok, kept):            ${buckets.sanctioned.length}  (TS_REPO_SANCTIONS)`
	);
	console.error(
		`  GAPS known (tsc+acorn valid):           ${buckets.gap_known.length}  (${category_line(by_category(buckets.gap_known))})`
	);
	console.error(
		`  beyond-acorn sanctioned (kept):         ${buckets.beyond_acorn_sanctioned.length}  (${category_line(by_category(buckets.beyond_acorn_sanctioned))})`
	);
	console.error(
		`  beyond-acorn known (to fix):            ${buckets.beyond_acorn_known.length}  (${category_line(by_category(buckets.beyond_acorn_known))})`
	);
	console.error(`  UNTRACKED over-rejections:              ${unexpected_total}  (GATES)`);

	for (const g of buckets.gap_unexpected) {
		console.error(`\n    ✗ UNTRACKED gap (acorn accepts): ${locate(g)}\n        ${g.tsv_error}`);
	}
	for (const g of buckets.beyond_acorn_unexpected) {
		console.error(
			`\n    ✗ UNTRACKED beyond-acorn (acorn rejects too): ${locate(g)}\n        ${g.tsv_error}`
		);
	}
	if (verbose) {
		const top_codes = [...over_acceptance_codes.entries()].sort((a, b) => b[1] - a[1]);
		console.error(
			`\n  over-acceptance by baseline code (files): ${top_codes.map(([c, n]) => `${c}=${n}`).join(' ')}`
		);
		for (const g of buckets.over_acceptance_parser) {
			console.error(
				`      · over-acceptance/parser ${basename(g.path)} — baseline ${g.baseline_codes.join(',')}; tsc parser ${g.tsc_parser_codes!.join(',')}`
			);
		}
		for (const g of buckets.script_goal) console.error(`      · script-goal ${locate(g)}`);
		for (const g of buckets.tsc_parser_rejects) {
			console.error(
				`      · tsc-parser-rejects ${locate(g)} — tsc ${g.tsc_parser_codes.join(',')}; tsv: ${g.tsv_error}`
			);
		}
		for (const g of buckets.sanctioned)
			console.error(`      · sanctioned ${locate(g)} — ${g.reason}`);
		for (const g of buckets.gap_known) {
			console.error(`      · known-gap [${g.category}] ${locate(g)} — ${g.reason}`);
		}
		for (const g of buckets.beyond_acorn_sanctioned) {
			console.error(`      · beyond-acorn/sanctioned [${g.category}] ${locate(g)} — ${g.reason}`);
		}
		for (const g of buckets.beyond_acorn_known) {
			console.error(
				`      · beyond-acorn/known [${g.category}] ${locate(g)} — ${g.reason}\n          ${g.tsv_error}`
			);
		}
	}

	if (json_mode) {
		const report = {
			root,
			oracle: 'tsc-baselines',
			scanned,
			declarations_graded,
			skipped: { ...skipped, ...discovery_skips },
			multi_file,
			accept_parity: buckets.accept_parity,
			reject_parity: buckets.reject_parity,
			over_acceptance: {
				parser: buckets.over_acceptance_parser,
				checker: buckets.over_acceptance_checker,
				codes: Object.fromEntries([...over_acceptance_codes.entries()].sort((a, b) => b[1] - a[1]))
			},
			script_goal: buckets.script_goal,
			tsc_parser_rejects: buckets.tsc_parser_rejects,
			sanctioned: buckets.sanctioned,
			gap_known: buckets.gap_known,
			gap_known_by_category: Object.fromEntries(by_category(buckets.gap_known)),
			gap_unexpected: buckets.gap_unexpected,
			beyond_acorn: {
				sanctioned: buckets.beyond_acorn_sanctioned,
				sanctioned_by_category: Object.fromEntries(by_category(buckets.beyond_acorn_sanctioned)),
				known: buckets.beyond_acorn_known,
				known_by_category: Object.fromEntries(by_category(buckets.beyond_acorn_known)),
				unexpected: buckets.beyond_acorn_unexpected
			}
		};
		Deno.stdout.writeSync(new TextEncoder().encode(JSON.stringify(report, null, '\t') + '\n'));
	}

	if (unexpected_total > 0) {
		console.error(
			`\nFAIL: ${unexpected_total} untracked over-rejection(s) — tsv rejects input tsc's baselines call valid. ` +
				`Fix the parser, or add a reasoned ledger entry (this file: KNOWN_GAPS / BEYOND_ACORN_*; ` +
				`lib/parse_sanctions.ts: TS_REPO_SANCTIONS) — docs/conformance_tsc.md §Triage.`
		);
		Deno.exit(1);
	}

	// Full-corpus-only hygiene (a subtree run legitimately grades a slice):
	if (is_default_root(root)) {
		// Ledger freshness: an entry matching nothing means its gap was fixed (delete
		// it) or upstream renamed the test (update it) — every list must mirror the
		// live corpus, like scan_audit's ALLOW list.
		const stale = Object.values(ledgers).flatMap((l) =>
			l.entries.filter((e) => !l.used.has(e.pattern)).map((e) => `${l.name}: ${e.pattern}`)
		);
		if (stale.length > 0) {
			console.error(
				`\nFAIL: ${stale.length} stale ledger entr${stale.length === 1 ? 'y' : 'ies'} — matched no over-rejection:\n` +
					stale.map((s) => `    · ${s}`).join('\n')
			);
			Deno.exit(1);
		}

		// Pinned counts (exact): the corpus is a deliberately-updated checkout, so any
		// move — a shrunken corpus, a collapsed oracle (accept-parity draining into
		// reject-parity/over-acceptance), or a tsv behavior change — must be re-pinned
		// deliberately, never absorbed. What each pin guards: lib/gate_counts.ts.
		const actual = {
			scanned,
			accept_parity: buckets.accept_parity,
			over_acceptance_parser: buckets.over_acceptance_parser.length,
			over_acceptance_checker: buckets.over_acceptance_checker.length,
			units_scanned: multi_file.units_scanned,
			units_accept_parity: multi_file.units_accept_parity
		};
		const pin_failures = (Object.keys(TS_REPO_PINS) as (keyof typeof TS_REPO_PINS)[])
			.filter((k) => actual[k] !== TS_REPO_PINS[k])
			.map((k) => `${k} ${actual[k]} ≠ pinned ${TS_REPO_PINS[k]}`);
		if (pin_failures.length > 0) {
			console.error(
				`\nFAIL: pinned count mismatch — ${pin_failures.join('; ')}. If this move is deliberate ` +
					`(checkout pull, behavior change), re-pin in lib/gate_counts.ts (see its update ritual).`
			);
			Deno.exit(1);
		}
	}

	console.error(
		`\nOK: no untracked over-rejections (every tsv over-rejection is ledgered, a goal artifact, or refused by tsc's own parser).`
	);
}

if (import.meta.main) {
	await run_ts_repo_compare();
}

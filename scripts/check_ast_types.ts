/**
 * Drift check for `crates/tsv_wasm/types/tsv_ast.d.ts`.
 *
 * The `.d.ts` is hand-maintained, so nothing but this gate holds it to what the wire-JSON
 * writers actually emit. It asks three questions, and they fail for different reasons:
 *
 * **A — writer conformance.** Invoke `tsv parse` on a curated set of snippets and type each
 * JSON output as a literal. TypeScript's excess-property checking on object literals catches
 * both directions of drift on the shapes those snippets reach: a field the `.d.ts` is missing
 * ("may only specify known properties"), and one it requires that the converter does not emit
 * ("Property 'X' is missing"). Type renames and value-type changes fall out the same way.
 * This arm grades the LIVE writer, so it is the one that fails the moment a `write_*` changes.
 *
 * **B — wire-type coverage.** Every `type` discriminant present anywhere in the committed
 * fixture wire must be declared in the `.d.ts`, or listed in `OPAQUE_WIRE_TYPES` below. This
 * is the cheap arm and the one with the widest reach: it is a set difference over text, no
 * type-checking involved. It exists because arm A can only be as right as its samples are
 * broad — `LogicalExpression` (`a && b`, one of the commonest nodes in JS) was absent from
 * the `.d.ts` entirely while this gate was green, because no curated snippet contained a
 * short-circuit operator.
 *
 * **C — fixture-corpus conformance.** Arm A over inputs nobody curated: a computed minimal
 * cover of the fixture tree's `expected*.json` — every discriminant the corpus contains, in
 * as few files as possible — typed against the same `.d.ts`. Two things make this stronger
 * than arm A rather than more of it. The committed `expected.json` is the CANONICAL parser's
 * output (`fixtures_update_parsed` regenerates it from Svelte / acorn-typescript /
 * `parseCss`), so this arm grades the `.d.ts` against the ORACLE, not against tsv's opinion
 * of itself; and its inputs are the whole corpus rather than a sample list, so a shape no one
 * thought to curate is still graded. `expected_ours.json` takes precedence where a fixture
 * declares a parser divergence — there tsv's wire is the contract.
 *
 * Arms A and C share one generated file and one `deno check` invocation.
 *
 * ⚠️ **Arm B is a NAME check, not a reachability check, and its scan is text-level.** Two
 * failure shapes follow, both live in the corpus today and both absorbed deliberately: a name
 * declared for an unrelated reason counts (`AttachedComment`'s `'Line' | 'Block'` covers the
 * CSS `Block` node's name for free — which is why `Block` is ALSO listed opaque below), and a
 * DATA key spelled `"type"` reads as a discriminant (`Boolean`, from the evaluated
 * `<svelte:options customElement>` props config). Arm C is what actually walks the typed
 * paths. The two are complementary in the usual way: B is broad and shallow, C is narrow and
 * deep.
 */

interface Sample {
	name: string;
	source: string;
	parser: 'typescript' | 'css' | 'svelte';
	type: 'Program' | 'StyleSheetFile' | 'Root';
}

const samples: Sample[] = [
	{
		name: 'ts_var_with_type_annotation',
		source: 'const x: number = 1;\nlet y = "two";',
		parser: 'typescript',
		type: 'Program'
	},
	{
		name: 'ts_optional_and_predicate_methods',
		source: [
			'abstract class A {',
			'	abstract m?(): void;',
			'}',
			'declare class C {',
			'	isA?(x: unknown): x is A;',
			'	g<T>(x: T): T;',
			'}'
		].join('\n'),
		parser: 'typescript',
		type: 'Program'
	},
	{
		name: 'ts_function_class_import',
		source: [
			'import { foo } from "./bar";',
			'export function add(a: number, b: number): number {',
			'	return a + b;',
			'}',
			'class C<T> extends Base implements I {',
			'	#priv: T;',
			'	constructor(public override readonly x: T) { super(); this.#priv = x; }',
			'}'
		].join('\n'),
		parser: 'typescript',
		type: 'Program'
	},
	{
		name: 'ts_directive_prologue',
		source: '"use strict";\nlet a = 1;',
		parser: 'typescript',
		type: 'Program'
	},
	{
		// `export default interface` — the TSInterfaceDeclaration member of the
		// ExportDefaultValue union.
		name: 'ts_export_default_interface',
		source: 'export default interface A {\n\ta: string;\n}',
		parser: 'typescript',
		type: 'Program'
	},
	{
		// Import attributes: bare `Identifier` key and quoted `Literal` key
		// (the `key: Identifier | Literal` union), on import + both re-export hosts.
		name: 'ts_import_attributes',
		source: [
			'import a from "./a" with { type: "json" };',
			'import b from "./b" with { "resolution-mode": "import" };',
			'export { c } from "./c" with { type: "json" };',
			'export * from "./d" with { type: "json" };'
		].join('\n'),
		parser: 'typescript',
		type: 'Program'
	},
	{
		// Over-rejection fixes: export-import-equals (`TSImportEqualsDeclaration`
		// with `isExport`), the UMD namespace export (`TSNamespaceExportDeclaration`),
		// and type-assertion assignment targets — the simple `=` left unwraps to the
		// inner target (`Identifier`), while `+=` keeps the assertion node.
		name: 'ts_assertion_targets_and_module_exports',
		source: [
			'export import NS = A.B;',
			'export as namespace Lib;',
			'(x as T) = 1;',
			'(y as U) += 2;',
			'[z as V] = arr;'
		].join('\n'),
		parser: 'typescript',
		type: 'Program'
	},
	{
		// String module specifiers (ES2022 `ModuleExportName : IdentifierName |
		// StringLiteral`): a string `imported`/`local`/`exported`, and the
		// `export * as 'str'` namespace name — the `ModuleExportName` union.
		name: 'ts_string_module_specifiers',
		source: [
			'import { "str a" as b } from "./a";',
			'export { c as "str c" } from "./c";',
			'export { "str d" as "str e" } from "./d";',
			'export * as "str f" from "./f";'
		].join('\n'),
		parser: 'typescript',
		type: 'Program'
	},
	{
		// Optional destructuring-pattern parameters (`[]?` / `{}?` / `[a]?: T`):
		// the `optional: true` flag on ArrayPattern/ObjectPattern, in a signature
		// member and the function-type path.
		name: 'ts_optional_pattern_params',
		source: [
			'interface I {',
			'\tm([]?): void;',
			'\tn({}?): void;',
			'\to([a]?: number[]): void;',
			'}',
			'type F = ([]?) => void;'
		].join('\n'),
		parser: 'typescript',
		type: 'Program'
	},
	{
		// Parameter decorators on non-identifier bindings — the `decorators` field
		// on ObjectPattern / ArrayPattern / AssignmentPattern (and, for a property,
		// the inner binding). acorn attaches a parameter's decorators to its
		// top-level binding node.
		name: 'ts_param_decorator_bindings',
		source: [
			'class A {',
			'\tconstructor(@d private a: number = 1) {}',
			'\tm(@d { a }: T, @e [b]: U, @f c = 1) {}',
			'}'
		].join('\n'),
		parser: 'typescript',
		type: 'Program'
	},
	{
		name: 'css_rule_at_media',
		source: '.foo { color: red; }\n@media (min-width: 600px) {\n\t.bar { padding: 1em 2em; }\n}',
		parser: 'css',
		type: 'StyleSheetFile'
	},
	{
		// The stylesheet-level `comments` array, POPULATED — the sibling above only
		// ever produces `[]`, which types the field's presence but no `CSSComment`.
		// Three readings in one sample: a structural comment (no `position`), one
		// lifted out of a declaration value, and one out of an at-rule prelude.
		name: 'css_comments',
		source:
			'/* top */\n.foo { color: /* v */ red; }\n@media /* p */ screen {\n\t.bar { top: 0; }\n}',
		parser: 'css',
		type: 'StyleSheetFile'
	},
	{
		name: 'svelte_script_element_style',
		source: [
			'<script lang="ts">',
			'\tlet x: number = 1;',
			'</script>',
			'',
			'<div class="a" on:click={() => x++}>{x}</div>',
			'',
			'<style>',
			'\t.a { color: red; }',
			'</style>'
		].join('\n'),
		parser: 'svelte',
		type: 'Root'
	},
	{
		name: 'svelte_blocks_and_directives',
		source: [
			'<script>',
			'\tlet items = [1, 2, 3];',
			'\tlet promise = fetch("/x");',
			'</script>',
			'',
			'{#each items as item, i (item)}',
			'\t<span use:enhance transition:fade>{i}: {item}</span>',
			'{/each}',
			'',
			'{#await promise}',
			'\tloading',
			'{:then value}',
			'\t{value}',
			'{:catch err}',
			'\t{err}',
			'{/await}'
		].join('\n'),
		parser: 'svelte',
		type: 'Root'
	}
];

async function parse(sample: Sample): Promise<string> {
	const cmd = new Deno.Command('cargo', {
		args: [
			'run',
			'--quiet',
			'-p',
			'tsv_cli',
			'--',
			'parse',
			'--content',
			sample.source,
			'--parser',
			sample.parser
		],
		stdout: 'piped',
		stderr: 'piped'
	});
	const { code, stdout, stderr } = await cmd.output();
	if (code !== 0) {
		const err = new TextDecoder().decode(stderr);
		throw new Error(`tsv parse failed for ${sample.name}:\n${err}`);
	}
	return new TextDecoder().decode(stdout).trim();
}

//
// The fixture corpus (arms B and C).
//

const FIXTURES_ROOT = 'tests/fixtures';
const DTS_PATH = 'crates/tsv_wasm/types/tsv_ast.d.ts';

/**
 * Root interface per fixture input kind. A fixture whose input name is not here is skipped
 * with a warning rather than guessed at — a new input kind should be a deliberate edit.
 */
const ROOT_TYPE: Record<string, Sample['type']> = {
	'input.svelte': 'Root',
	'input.svelte.ts': 'Program',
	'input.ts': 'Program',
	'input.css': 'StyleSheetFile'
};

/**
 * Wire `type` discriminants the `.d.ts` deliberately does NOT declare. Two kinds, each with
 * its own reason:
 *
 * - **The CSS node vocabulary** (every entry but `Boolean`): `StyleSheet.children` /
 *   `StyleSheetFile.children` / `StyleSheet.attributes` are `unknown[]` by design ("their
 *   precise shape is not currently mirrored in this file"), so the CSS tree below the
 *   stylesheet root has no interfaces to name. `Block` belongs here even though the name
 *   check would pass without it — `'Line' | 'Block'` covers the name by accident (see the
 *   arm-B warning above) — because `wire_field_slots` keys its opaque-region filter on THIS
 *   set, and membership is the statement that the node is opaque on purpose.
 * - **`Boolean` is not a node at all**: it is a DATA key inside Svelte's evaluated
 *   `<svelte:options customElement>` props config (`props: {x: {type: 'Boolean'}}`), which
 *   the text-level discriminant scan cannot tell from a discriminator. It sits under the
 *   `unknown`-typed `SvelteOptions.customElement`, and typing that field would retire the
 *   entry.
 *
 * This is a snapshot of a deliberate boundary, not a bug list: shrinking the CSS side means
 * typing the CSS AST, and a name may only be ADDED here alongside a decision to leave that
 * region opaque. Every discriminant the corpus produces that is not here must be declared.
 */
const OPAQUE_WIRE_TYPES: ReadonlySet<string> = new Set([
	'Atrule',
	'AttributeSelector',
	'Block',
	'Boolean',
	'ClassSelector',
	'Combinator',
	'ComplexSelector',
	'Declaration',
	'IdSelector',
	'NestingSelector',
	'Nth',
	'Percentage',
	'PseudoClassSelector',
	'PseudoElementSelector',
	'RelativeSelector',
	'Rule',
	'SelectorList',
	'TypeSelector'
]);

/**
 * Floors, in the spirit of the audit gates' vacuity guards: a walk that resolves nothing, or
 * a corpus that quietly shrank, must fail rather than pass by absence. Both are minimums, so
 * ordinary fixture growth never touches them.
 */
const FIXTURES_MIN = 3500;
const DISCRIMINANTS_MIN = 180;
const SLOTS_MIN = 2400;

interface FixtureWire {
	path: string;
	root: Sample['type'];
	bytes: number;
	/** The `type` discriminants this wire contains (arm B). */
	types: Set<string>;
	/** The `ParentType.key->ChildType` slots this wire contains (arm C's cover target). */
	positions: Set<string>;
}

/** Every `type: 'A' | 'B'` literal the `.d.ts` declares, union spellings included. */
function declared_discriminants(dts: string): Set<string> {
	const out = new Set<string>();
	const decl = /\btype:\s*\|?\s*((?:'[A-Za-z]+'\s*\|\s*)*'[A-Za-z]+')/g;
	for (const m of dts.matchAll(decl)) {
		for (const lit of m[1].matchAll(/'([A-Za-z]+)'/g)) out.add(lit[1]);
	}
	return out;
}

/** The `type` discriminants a wire JSON text contains. Text, not a parse — this runs over the
 * whole corpus and only the selected files are ever parsed. */
function wire_discriminants(json: string): Set<string> {
	const out = new Set<string>();
	for (const m of json.matchAll(/"type"\s*:\s*"([A-Za-z]+)"/g)) out.add(m[1]);
	return out;
}

/**
 * The `ParentType.key -> ChildType` field SLOTS a wire tree contains — the unit arm C covers.
 *
 * ⚠️ **Granularity is the whole design of this gate, and it has been wrong twice.** Both
 * excess-property and value-type checking fire per (interface, key, value shape), so a cover
 * is only as strong as the unit it spans. Measured, in order:
 *
 * - A cover over node **TYPES** (99 files) missed a wrong `TSInterfaceDeclaration.extends` —
 *   the fixture supplying `TSExpressionWithTypeArguments` did so through a class `implements`
 *   clause, so the node was reached and the position was not.
 * - A cover over **POSITIONS** (`ParentType.key`, 267 files) caught that, and still missed
 *   three more: a loc-less `TSTypeAnnotation` on block bindings, two attachment hosts, and a
 *   `TSExpressionWithTypeArguments.expression` that is a `TSQualifiedName`. Each is a position
 *   already covered by some *other* value shape.
 * - Covering the **slot** — position plus the child's own discriminator — catches all of them.
 *
 * The cost is real and was measured before choosing: 267 files / ~1 s for positions vs 770
 * files / ~18 s for slots. Going back is a one-line change (drop the `->${child}` below), but
 * the trade it buys is exactly the class of bug this gate exists for.
 *
 * `#scalar` / `#empty` / `#obj` stand in where a value has no discriminator of its own, so a
 * scalar-vs-node change at a slot still registers as a distinct slot.
 *
 * Slots inside an OPAQUE region (parent or child named in `OPAQUE_WIRE_TYPES`) are skipped:
 * the typed side is `unknown` there, so covering them spends `deno check` time grading
 * nothing. The filter keys on the live set, so the day a region is typed (its names leave the
 * set) its slots re-enter the cover automatically. Safe in both directions today because
 * every opaque name occurs only inside an `unknown`-typed region — the stale-entry check is
 * what keeps that true.
 */
function wire_field_slots(json: unknown): Set<string> {
	const out = new Set<string>();
	const shape = (v: unknown): string => {
		if (v === null || typeof v !== 'object') return '#scalar';
		if (Array.isArray(v)) return '#scalar';
		const t = (v as Record<string, unknown>).type;
		return typeof t === 'string' ? t : '#obj';
	};
	const add = (parent: string, k: string, child: string): void => {
		if (OPAQUE_WIRE_TYPES.has(parent) || OPAQUE_WIRE_TYPES.has(child)) return;
		out.add(`${parent}.${k}->${child}`);
	};
	const visit = (n: unknown): void => {
		if (Array.isArray(n)) {
			for (const e of n) visit(e);
			return;
		}
		if (n === null || typeof n !== 'object') return;
		const obj = n as Record<string, unknown>;
		const parent = typeof obj.type === 'string' ? obj.type : null;
		for (const [k, v] of Object.entries(obj)) {
			if (parent !== null && k !== 'type') {
				if (Array.isArray(v)) {
					if (v.length === 0) add(parent, k, '#empty');
					for (const e of v) add(parent, k, shape(e));
				} else {
					add(parent, k, shape(v));
				}
			}
			visit(v);
		}
	};
	visit(json);
	return out;
}

async function collect_fixture_wires(): Promise<FixtureWire[]> {
	const out: FixtureWire[] = [];
	const walk = async (dir: string): Promise<void> => {
		const entries = [];
		for await (const e of Deno.readDir(dir)) entries.push(e);
		entries.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
		const names = new Set(entries.filter((e) => e.isFile).map((e) => e.name));
		// `expected_ours.json` is tsv's wire where a fixture declares a parser divergence, and
		// the `.d.ts` describes what tsv emits — so it wins over the canonical `expected.json`.
		const pick = names.has('expected_ours.json')
			? 'expected_ours.json'
			: names.has('expected.json')
				? 'expected.json'
				: null;
		const input = [...names].find((n) => n.startsWith('input.') && !n.startsWith('input_invalid'));
		if (pick && input) {
			const root = ROOT_TYPE[input];
			if (!root) {
				console.warn(`  ⚠ unknown fixture input kind, skipped: ${dir}/${input}`);
			} else {
				const path = `${dir}/${pick}`;
				const text = await Deno.readTextFile(path);
				// A no-oracle marker payload (`{"error": …}`) types nothing.
				if (!/^\s*\{\s*"error"/.test(text)) {
					out.push({
						path,
						root,
						bytes: text.length,
						types: wire_discriminants(text),
						positions: wire_field_slots(JSON.parse(text))
					});
				}
			}
		}
		for (const e of entries) {
			if (e.isDirectory) await walk(`${dir}/${e.name}`);
		}
	};
	await walk(FIXTURES_ROOT);
	return out;
}

/**
 * Greedy minimal cover: repeatedly take the fixture carrying the most still-uncovered
 * discriminants PER BYTE, so the generated file stays small (weighting by count alone picks
 * the corpus's biggest files and inflates it ~7x). Ties break on path, so the selection is
 * deterministic and a failure reproduces.
 */
function cover(wires: FixtureWire[], wanted: Set<string>): FixtureWire[] {
	const uncovered = new Set(wanted);
	const chosen: FixtureWire[] = [];
	while (uncovered.size > 0) {
		let best: FixtureWire | null = null;
		let best_score = 0;
		let best_gain = 0;
		for (const w of wires) {
			let gain = 0;
			for (const t of w.positions) if (uncovered.has(t)) gain++;
			if (gain === 0) continue;
			const score = gain / Math.max(w.bytes, 1);
			if (
				score > best_score ||
				(score === best_score && gain > best_gain) ||
				(score === best_score && gain === best_gain && best !== null && w.path < best.path)
			) {
				best = w;
				best_score = score;
				best_gain = gain;
			}
		}
		if (!best) break;
		chosen.push(best);
		for (const t of best.positions) uncovered.delete(t);
	}
	chosen.sort((a, b) => (a.path < b.path ? -1 : 1));
	return chosen;
}

const dts = await Deno.readTextFile(DTS_PATH);
const declared = declared_discriminants(dts);

console.log(`Scanning ${FIXTURES_ROOT} …`);
const wires = await collect_fixture_wires();
const corpus_types = new Set<string>();
for (const w of wires) for (const t of w.types) corpus_types.add(t);

if (wires.length < FIXTURES_MIN) {
	console.error(
		`Only ${wires.length} fixture wire(s) resolved (floor ${FIXTURES_MIN}). The corpus walk is` +
			` broken or the tree moved — refusing to grade a shrunken corpus.`
	);
	Deno.exit(1);
}
if (corpus_types.size < DISCRIMINANTS_MIN) {
	console.error(
		`Only ${corpus_types.size} wire discriminant(s) found (floor ${DISCRIMINANTS_MIN}).` +
			` The extraction is broken — refusing to grade.`
	);
	Deno.exit(1);
}

// Arm B — every discriminant the wire produces is declared, or deliberately opaque.
const undeclared = [...corpus_types].filter((t) => !declared.has(t) && !OPAQUE_WIRE_TYPES.has(t));
undeclared.sort();
const stale_opaque = [...OPAQUE_WIRE_TYPES].filter((t) => !corpus_types.has(t));
stale_opaque.sort();
if (undeclared.length > 0 || stale_opaque.length > 0) {
	console.error('');
	if (undeclared.length > 0) {
		console.error(`Wire-type coverage gap — ${undeclared.length} discriminant(s) the fixture`);
		console.error(`corpus produces but ${DTS_PATH} does not declare:`);
		for (const t of undeclared) console.error(`  ${t}`);
		console.error('');
		console.error('Declare each one, or — if the node is deliberately opaque — add it to');
		console.error('OPAQUE_WIRE_TYPES with the reason.');
	}
	if (stale_opaque.length > 0) {
		console.error(`Stale OPAQUE_WIRE_TYPES entr(ies) — no longer produced by the corpus:`);
		for (const t of stale_opaque) console.error(`  ${t}`);
		console.error('Remove them; an opaque entry that fires on nothing hides the next one.');
	}
	Deno.exit(1);
}
console.log(
	`Wire-type coverage: ${corpus_types.size} discriminant(s) across ${wires.length} fixture` +
		` wire(s) — ${corpus_types.size - OPAQUE_WIRE_TYPES.size} declared,` +
		` ${OPAQUE_WIRE_TYPES.size} deliberately opaque.`
);

// Arm C — the minimal cover, typed against the .d.ts alongside the curated samples.
const corpus_positions = new Set<string>();
for (const w of wires) for (const t of w.positions) corpus_positions.add(t);
if (corpus_positions.size < SLOTS_MIN) {
	console.error(
		`Only ${corpus_positions.size} wire field slot(s) found (floor ${SLOTS_MIN}).` +
			` The extraction is broken — refusing to grade.`
	);
	Deno.exit(1);
}
const selected = cover(wires, corpus_positions);
if (selected.length === 0) {
	console.error('Cover selection produced no fixtures — refusing to type-check nothing.');
	Deno.exit(1);
}
console.log(
	`Cover: ${selected.length} fixture(s) span all ${corpus_positions.size} gradable wire field slot(s).`
);

const gen_path = 'scripts/.drift_check.gen.ts';

const header = [
	'// Auto-generated by scripts/check_ast_types.ts — do not edit.',
	'// Tests that tsv_ast.d.ts accepts the shapes produced by `tsv parse` (curated samples)',
	'// and the shapes committed in the fixture corpus (computed cover).',
	"import type { Program, Root, StyleSheetFile } from '../crates/tsv_wasm/types/tsv_ast.d.ts';",
	''
];

console.log(`Parsing ${samples.length} curated sample(s)...`);
const jsons = await Promise.all(samples.map(parse));

const body = samples.map((sample, i) => `const _${sample.name}: ${sample.type} = ${jsons[i]};\n`);

for (const [i, w] of selected.entries()) {
	// Re-serialize compactly: the committed files are tab-indented, which is ~2x the bytes for
	// `deno check` to tokenize and buys nothing here.
	const compact = JSON.stringify(JSON.parse(await Deno.readTextFile(w.path)));
	body.push(`// ${w.path}\nconst _cover_${i}: ${w.root} = ${compact};\n`);
}

await Deno.writeTextFile(gen_path, [...header, ...body].join('\n'));
console.log(`Wrote ${gen_path}`);

console.log(`Running \`deno check ${gen_path}\`...`);
const check = new Deno.Command('deno', {
	args: ['check', gen_path],
	stdout: 'inherit',
	stderr: 'inherit'
});
const { code } = await check.output();

if (code !== 0) {
	console.error('');
	console.error(`Drift detected. ${gen_path} left in place for inspection.`);
	console.error(`Update crates/tsv_wasm/types/tsv_ast.d.ts to match the actual`);
	console.error(`shape, or fix the convert layer if the .d.ts is correct.`);
	Deno.exit(1);
}

await Deno.remove(gen_path);
console.log(
	`OK — no drift across ${samples.length} curated sample(s) + ${selected.length} fixture cover file(s).`
);

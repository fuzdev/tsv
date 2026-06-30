/**
 * Drift check for `crates/tsv_wasm/types/tsv_ast.d.ts`.
 *
 * Invokes `tsv parse` on a curated set of source snippets, embeds each JSON
 * output as a typed literal in a generated TS file, then runs `deno check`.
 * TypeScript's excess-property checking on object literals catches the two
 * directions of drift between the hand-maintained `.d.ts` and the actual
 * `convert_ast_json` output:
 *
 *   - `.d.ts` missing a field that the converter emits  → "may only specify
 *     known properties" error on the literal.
 *   - `.d.ts` requires a field the converter does not emit → "Property
 *     'X' is missing" error.
 *
 * Type renames and value-type changes are caught the same way.
 *
 * The curated set is small by design — coverage trades off against
 * invocation cost. Add a sample when a previously-uncovered AST node
 * regresses; the goal is structural coverage, not exhaustive fixture-style
 * snapshot testing.
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
		type: 'Program',
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
			'}',
		].join('\n'),
		parser: 'typescript',
		type: 'Program',
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
			'}',
		].join('\n'),
		parser: 'typescript',
		type: 'Program',
	},
	{
		name: 'ts_directive_prologue',
		source: '"use strict";\nlet a = 1;',
		parser: 'typescript',
		type: 'Program',
	},
	{
		// `export default interface` — the TSInterfaceDeclaration member of the
		// ExportDefaultValue union.
		name: 'ts_export_default_interface',
		source: 'export default interface A {\n\ta: string;\n}',
		parser: 'typescript',
		type: 'Program',
	},
	{
		// Import attributes: bare `Identifier` key and quoted `Literal` key
		// (the `key: Identifier | Literal` union), on import + both re-export hosts.
		name: 'ts_import_attributes',
		source: [
			'import a from "./a" with { type: "json" };',
			'import b from "./b" with { "resolution-mode": "import" };',
			'export { c } from "./c" with { type: "json" };',
			'export * from "./d" with { type: "json" };',
		].join('\n'),
		parser: 'typescript',
		type: 'Program',
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
			'[z as V] = arr;',
		].join('\n'),
		parser: 'typescript',
		type: 'Program',
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
			'export * as "str f" from "./f";',
		].join('\n'),
		parser: 'typescript',
		type: 'Program',
	},
	{
		name: 'css_rule_at_media',
		source: '.foo { color: red; }\n@media (min-width: 600px) {\n\t.bar { padding: 1em 2em; }\n}',
		parser: 'css',
		type: 'StyleSheetFile',
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
			'</style>',
		].join('\n'),
		parser: 'svelte',
		type: 'Root',
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
			'{/await}',
		].join('\n'),
		parser: 'svelte',
		type: 'Root',
	},
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
			sample.parser,
		],
		stdout: 'piped',
		stderr: 'piped',
	});
	const { code, stdout, stderr } = await cmd.output();
	if (code !== 0) {
		const err = new TextDecoder().decode(stderr);
		throw new Error(`tsv parse failed for ${sample.name}:\n${err}`);
	}
	return new TextDecoder().decode(stdout).trim();
}

const gen_path = 'scripts/.drift_check.gen.ts';
const decline_path = gen_path; // kept for cleanup symmetry below

const header = [
	'// Auto-generated by scripts/check_ast_types.ts — do not edit.',
	'// Tests that tsv_ast.d.ts accepts the shapes produced by `tsv parse`.',
	"import type { Program, Root, StyleSheetFile } from '../crates/tsv_wasm/types/tsv_ast.d.ts';",
	'',
];

console.log(`Parsing ${samples.length} sample(s)...`);
const jsons = await Promise.all(samples.map(parse));

const body = samples.map((sample, i) => `const _${sample.name}: ${sample.type} = ${jsons[i]};\n`);

await Deno.writeTextFile(gen_path, [...header, ...body].join('\n'));
console.log(`Wrote ${gen_path}`);

console.log(`Running \`deno check ${gen_path}\`...`);
const check = new Deno.Command('deno', {
	args: ['check', gen_path],
	stdout: 'inherit',
	stderr: 'inherit',
});
const { code } = await check.output();

if (code !== 0) {
	console.error('');
	console.error(`Drift detected. ${gen_path} left in place for inspection.`);
	console.error(`Update crates/tsv_wasm/types/tsv_ast.d.ts to match the actual`);
	console.error(`shape, or fix the convert layer if the .d.ts is correct.`);
	Deno.exit(1);
}

await Deno.remove(decline_path);
console.log(`OK — no drift across ${samples.length} sample(s).`);

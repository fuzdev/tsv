/**
 * Hand-maintained TypeScript declarations for the tsv wire JSON (the public
 * AST each parse export returns).
 *
 * Mirrors what the wire-JSON writers emit:
 *   - crates/tsv_ts/src/ast/convert/write/
 *   - crates/tsv_css/src/ast/convert/write.rs
 *   - crates/tsv_svelte/src/ast/convert/write.rs
 *
 * Bundled inside `@fuzdev/tsv_parse_wasm` and `@fuzdev/tsv_wasm`. Any change
 * to the JSON a writer emits must be mirrored here — see
 * `crates/tsv_wasm/CLAUDE.md` for the maintenance checklist.
 *
 * Field names are exactly the keys the writer emits. A field emitted only
 * conditionally (`if let Some(..)` / `if flag`) is optional (`T?`); a field
 * always emitted with a `null` fallback (`write_or_null`) is `T | null`.
 */

//
// Foundational types (reused across TypeScript, CSS, and Svelte ASTs).
//

/** Line/column position. `character` is a byte offset that only appears on identifiers Svelte creates directly. */
export interface Position {
	line: number;
	column: number;
	/** Omitted from JSON when absent. */
	character?: number;
}

/** Range with start/end positions, attached to every AST node via `loc`. */
export interface SourceLocation {
	start: Position;
	end: Position;
}

/** Decorator applied to a class or class member: `@expression`. */
export interface Decorator {
	type: 'Decorator';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
}

/**
 * Literal value: string, number, boolean, null, or BigInt.
 * Numeric values with no fractional part serialize as integers.
 */
export interface Literal {
	type: 'Literal';
	start: number;
	end: number;
	loc: SourceLocation;
	value: string | number | boolean | bigint | null;
	raw: string;
	/** Present only for BigInt literals (e.g., `1n`). Omitted otherwise. */
	bigint?: string;
}

/** Identifier name (variable, parameter, property name, etc.). */
export interface Identifier {
	type: 'Identifier';
	start: number;
	end: number;
	loc: SourceLocation;
	name: string;
	/** Optional parameter marker (`x?`). Omitted from JSON when false. */
	optional?: boolean;
	/** Omitted from JSON when absent. */
	typeAnnotation?: TSTypeAnnotation;
	/** Decorators on this parameter. Omitted from JSON when empty. */
	decorators?: Decorator[];
}

/** Private class member identifier: `#name`. Name excludes the `#`. */
export interface PrivateIdentifier {
	type: 'PrivateIdentifier';
	start: number;
	end: number;
	loc: SourceLocation;
	name: string;
}

/**
 * Acorn `Program` node — root of a TypeScript/JS source file.
 *
 * When the source is a Svelte `<script>` block (non-`lang="ts"`), some
 * import/export fields use the SvelteScript schema variant (notably
 * `importKind`/`exportKind` may be omitted). The resulting shape is a
 * subset of this interface; no extra fields are added.
 */
export interface Program {
	type: 'Program';
	start: number;
	end: number;
	loc: SourceLocation;
	body: Statement[];
	sourceType: string;
}

//
// TypeScript AST — statements
//

export type Statement =
	| ExpressionStatement
	| VariableDeclaration
	| TSTypeAliasDeclaration
	| TSInterfaceDeclaration
	| TSDeclareFunction
	| TSEnumDeclaration
	| TSModuleDeclaration
	| ReturnStatement
	| BlockStatement
	| FunctionDeclaration
	| ClassDeclaration
	| ExportNamedDeclaration
	| ExportDefaultDeclaration
	| ExportAllDeclaration
	| TSExportAssignment
	| TSNamespaceExportDeclaration
	| ImportDeclaration
	| TSImportEqualsDeclaration
	| IfStatement
	| ForStatement
	| ForInStatement
	| ForOfStatement
	| WhileStatement
	| DoWhileStatement
	| SwitchStatement
	| TryStatement
	| ThrowStatement
	| BreakStatement
	| ContinueStatement
	| LabeledStatement
	| EmptyStatement
	| DebuggerStatement;

export interface ExpressionStatement {
	type: 'ExpressionStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
	/** Present only for directive prologue entries: raw string contents without quotes. */
	directive?: string;
}

export interface BlockStatement {
	type: 'BlockStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	body: Statement[];
}

export interface FunctionDeclaration {
	type: 'FunctionDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	id: Identifier | null;
	expression: boolean;
	generator: boolean;
	async: boolean;
	typeParameters?: TSTypeParameterDeclaration;
	params: Expression[];
	returnType?: TSTypeAnnotation;
	body: BlockStatement;
}

export interface ReturnStatement {
	type: 'ReturnStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	argument: Expression | null;
}

export interface IfStatement {
	type: 'IfStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	test: Expression;
	consequent: Statement;
	alternate: Statement | null;
}

export interface ForStatement {
	type: 'ForStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	init: ForInit | null;
	test: Expression | null;
	update: Expression | null;
	body: Statement;
}

export type ForInit = VariableDeclaration | Expression;

export interface ForInStatement {
	type: 'ForInStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	left: ForInOfLeft;
	right: Expression;
	body: Statement;
}

export interface ForOfStatement {
	type: 'ForOfStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	await: boolean;
	left: ForInOfLeft;
	right: Expression;
	body: Statement;
}

export type ForInOfLeft = VariableDeclaration | Expression;

export interface WhileStatement {
	type: 'WhileStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	test: Expression;
	body: Statement;
}

export interface DoWhileStatement {
	type: 'DoWhileStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	body: Statement;
	test: Expression;
}

export interface SwitchStatement {
	type: 'SwitchStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	discriminant: Expression;
	cases: SwitchCase[];
}

export interface SwitchCase {
	type: 'SwitchCase';
	start: number;
	end: number;
	loc: SourceLocation;
	test: Expression | null;
	consequent: Statement[];
}

export interface TryStatement {
	type: 'TryStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	block: BlockStatement;
	handler: CatchClause | null;
	finalizer: BlockStatement | null;
}

export interface CatchClause {
	type: 'CatchClause';
	start: number;
	end: number;
	loc: SourceLocation;
	param: Expression | null;
	body: BlockStatement;
}

export interface ThrowStatement {
	type: 'ThrowStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	argument: Expression;
}

export interface BreakStatement {
	type: 'BreakStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	label: Identifier | null;
}

export interface ContinueStatement {
	type: 'ContinueStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	label: Identifier | null;
}

export interface LabeledStatement {
	type: 'LabeledStatement';
	start: number;
	end: number;
	loc: SourceLocation;
	label: Identifier;
	body: Statement;
}

export interface EmptyStatement {
	type: 'EmptyStatement';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface DebuggerStatement {
	type: 'DebuggerStatement';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface VariableDeclaration {
	type: 'VariableDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	declarations: VariableDeclarator[];
	kind: string;
	/** Omitted from JSON when false. */
	declare?: boolean;
}

export interface VariableDeclarator {
	type: 'VariableDeclarator';
	start: number;
	end: number;
	loc: SourceLocation;
	id: Expression;
	/** Definite assignment assertion (`!`). Omitted from JSON when false. */
	definite?: boolean;
	init: Expression | null;
}

//
// TypeScript AST — expressions
//

export type Expression =
	| Literal
	| Identifier
	| PrivateIdentifier
	| ObjectExpression
	| ArrayExpression
	| UnaryExpression
	| UpdateExpression
	| BinaryExpression
	| CallExpression
	| NewExpression
	| MemberExpression
	| ConditionalExpression
	| ArrowFunctionExpression
	| FunctionExpression
	| ClassExpression
	| SpreadElement
	| TemplateLiteral
	| TaggedTemplateExpression
	| AwaitExpression
	| YieldExpression
	| SequenceExpression
	| RegexLiteral
	| ThisExpression
	| Super
	| AssignmentExpression
	| ObjectPattern
	| ArrayPattern
	| AssignmentPattern
	| RestElement
	| TSTypeAssertion
	| TSAsExpression
	| TSSatisfiesExpression
	| TSInstantiationExpression
	| TSNonNullExpression
	| ImportExpression
	| MetaProperty
	| TSParameterProperty
	| ParenthesizedExpression
	| ChainExpression;

export interface ObjectExpression {
	type: 'ObjectExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	properties: ObjectProperty[];
}

export type ObjectProperty = Property | SpreadElement;

export interface ArrayExpression {
	type: 'ArrayExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	elements: (Expression | null)[];
}

export interface UnaryExpression {
	type: 'UnaryExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	operator: string;
	prefix: boolean;
	argument: Expression;
}

/** Update expression: `++x`, `x++`, `--x`, `x--`. */
export interface UpdateExpression {
	type: 'UpdateExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	operator: string;
	prefix: boolean;
	argument: Expression;
}

export interface BinaryExpression {
	type: 'BinaryExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	left: Expression;
	operator: string;
	right: Expression;
}

export interface CallExpression {
	type: 'CallExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	callee: Expression;
	arguments: Expression[];
	typeArguments?: TSTypeParameterInstantiation;
	/** acorn-typescript omits `optional` when `typeArguments` is present or in decorator contexts. */
	optional?: boolean;
}

/** `new Foo()`, `new Map<K, V>()`. */
export interface NewExpression {
	type: 'NewExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	callee: Expression;
	arguments: Expression[];
	typeArguments?: TSTypeParameterInstantiation;
}

/** Dynamic import: `import('mod')` or `import('mod', {with: {type: 'json'}})`. */
export interface ImportExpression {
	type: 'ImportExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	source: Expression;
	/** Import phase (`'source'`/`'defer'`) for `import.source(…)` / `import.defer(…)`; omitted otherwise. */
	phase?: 'source' | 'defer';
	/** Import attribute arguments. Omitted from JSON when empty. */
	arguments?: Expression[];
}

/** `import.meta` or `new.target`. */
export interface MetaProperty {
	type: 'MetaProperty';
	start: number;
	end: number;
	loc: SourceLocation;
	meta: Identifier;
	property: Identifier;
}

export interface MemberExpression {
	type: 'MemberExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	object: Expression;
	property: Expression;
	computed: boolean;
	/** acorn omits `optional` in decorator-expression contexts. */
	optional?: boolean;
}

/** Optional chaining wrapper: `a?.b`, `a?.b()`, `a?.b.c.d()`. */
export interface ChainExpression {
	type: 'ChainExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
}

export interface ConditionalExpression {
	type: 'ConditionalExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	test: Expression;
	consequent: Expression;
	alternate: Expression;
}

export interface ArrowFunctionExpression {
	type: 'ArrowFunctionExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Always null for arrow functions. */
	id: null;
	expression: boolean;
	generator: boolean;
	async: boolean;
	params: Expression[];
	body: ArrowFunctionBody;
	typeParameters?: TSTypeParameterDeclaration;
	returnType?: TSTypeAnnotation;
}

export type ArrowFunctionBody = Expression | BlockStatement;

export interface SpreadElement {
	type: 'SpreadElement';
	start: number;
	end: number;
	loc: SourceLocation;
	argument: Expression;
}

export interface TemplateLiteral {
	type: 'TemplateLiteral';
	start: number;
	end: number;
	loc: SourceLocation;
	expressions: Expression[];
	quasis: TemplateElement[];
}

export interface TemplateElement {
	type: 'TemplateElement';
	start: number;
	end: number;
	loc: SourceLocation;
	value: TemplateElementValue;
	tail: boolean;
}

export interface TemplateElementValue {
	raw: string;
	/** `null` for invalid escape sequences in tagged templates. */
	cooked: string | null;
}

export interface TaggedTemplateExpression {
	type: 'TaggedTemplateExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	tag: Expression;
	quasi: TemplateLiteral;
	typeArguments?: TSTypeParameterInstantiation;
}

export interface AwaitExpression {
	type: 'AwaitExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	argument: Expression;
}

export interface YieldExpression {
	type: 'YieldExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	delegate: boolean;
	argument: Expression | null;
}

export interface SequenceExpression {
	type: 'SequenceExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	expressions: Expression[];
}

/**
 * Parenthesized expression: `(expr)`. Only emitted for `{#snippet}` parameters,
 * where Svelte parses with acorn's `preserveParens` and skips `remove_parens`,
 * so the parens survive in the AST (e.g. a default `c = (2, 3)`). Every other
 * grouping paren is discarded (the public AST is otherwise paren-free). `start`
 * and `end` cover the parentheses; `expression` keeps its own paren-free span.
 */
export interface ParenthesizedExpression {
	type: 'ParenthesizedExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
}

/** Regex literal: `/pat/flags`. Serializes with `type: "Literal"` to match acorn. */
export interface RegexLiteral {
	type: 'Literal';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Always serializes as an empty object (regex can't be represented in JSON). */
	value: unknown;
	raw: string;
	regex: RegexValue;
}

export interface RegexValue {
	pattern: string;
	flags: string;
}

export interface ThisExpression {
	type: 'ThisExpression';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface Super {
	type: 'Super';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface AssignmentExpression {
	type: 'AssignmentExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	operator: string;
	left: Expression;
	right: Expression;
}

/** Object property (also used inside object patterns). */
export interface Property {
	type: 'Property';
	start: number;
	end: number;
	loc: SourceLocation;
	method: boolean;
	shorthand: boolean;
	computed: boolean;
	key: Expression;
	kind: string;
	value: Expression;
}

/** `<Type>expr` angle-bracket type assertion. */
export interface TSTypeAssertion {
	type: 'TSTypeAssertion';
	start: number;
	end: number;
	loc: SourceLocation;
	typeAnnotation: TSType;
	expression: Expression;
}

/** `expr as Type` or `expr as const`. */
export interface TSAsExpression {
	type: 'TSAsExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
	typeAnnotation: TSType;
}

/** `expr satisfies Type`. */
export interface TSSatisfiesExpression {
	type: 'TSSatisfiesExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
	typeAnnotation: TSType;
}

/** Instantiation expression: `f<T>`. */
export interface TSInstantiationExpression {
	type: 'TSInstantiationExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
	typeArguments: TSTypeParameterInstantiation;
}

/** Non-null assertion: `expr!`. */
export interface TSNonNullExpression {
	type: 'TSNonNullExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
}

//
// TypeScript AST — classes
//

export interface ClassDeclaration {
	type: 'ClassDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	decorators?: Decorator[];
	declare?: boolean;
	abstract?: boolean;
	id: Identifier | null;
	typeParameters?: TSTypeParameterDeclaration;
	superClass: Expression | null;
	superTypeParameters?: TSTypeParameterInstantiation;
	implements?: TSExpressionWithTypeArguments[];
	body: ClassBody;
}

export interface ClassExpression {
	type: 'ClassExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	decorators?: Decorator[];
	abstract?: boolean;
	id: Identifier | null;
	typeParameters?: TSTypeParameterDeclaration;
	superClass: Expression | null;
	superTypeParameters?: TSTypeParameterInstantiation;
	implements?: TSExpressionWithTypeArguments[];
	body: ClassBody;
}

export interface ClassBody {
	type: 'ClassBody';
	start: number;
	end: number;
	loc: SourceLocation;
	body: ClassMember[];
}

export type ClassMember =
	| MethodDefinition
	| PropertyDefinition
	| StaticBlock
	| TSIndexSignature;

/** Static initialization block: `static { ... }` (ES2022). */
export interface StaticBlock {
	type: 'StaticBlock';
	start: number;
	end: number;
	loc: SourceLocation;
	body: Statement[];
}

export interface MethodDefinition {
	type: 'MethodDefinition';
	start: number;
	end: number;
	loc: SourceLocation;
	decorators?: Decorator[];
	accessibility?: string;
	abstract?: boolean;
	static: boolean;
	/** Omitted from JSON when false. */
	override?: boolean;
	/** Optional method (`m?()`); omitted from JSON when false. */
	optional?: boolean;
	computed: boolean;
	key: Expression;
	kind: string;
	typeParameters?: TSTypeParameterDeclaration;
	value: MethodValue;
}

export type MethodValue = FunctionExpression | TSDeclareMethod;

/** Abstract method or overload signature (no body). */
export interface TSDeclareMethod {
	type: 'TSDeclareMethod';
	start: number;
	end: number;
	loc: SourceLocation;
	id: Identifier | null;
	expression: boolean;
	generator: boolean;
	async: boolean;
	params: Expression[];
	returnType?: TSTypeAnnotation;
}

export interface PropertyDefinition {
	type: 'PropertyDefinition';
	start: number;
	end: number;
	loc: SourceLocation;
	decorators?: Decorator[];
	abstract?: boolean;
	accessor?: boolean;
	accessibility?: string;
	readonly?: boolean;
	override?: boolean;
	declare?: boolean;
	static: boolean;
	computed: boolean;
	key: Expression;
	optional?: boolean;
	definite?: boolean;
	typeAnnotation?: TSTypeAnnotation;
	value: Expression | null;
}

export interface FunctionExpression {
	type: 'FunctionExpression';
	start: number;
	end: number;
	loc: SourceLocation;
	id: Identifier | null;
	expression: boolean;
	generator: boolean;
	async: boolean;
	typeParameters?: TSTypeParameterDeclaration;
	params: Expression[];
	returnType?: TSTypeAnnotation;
	body: BlockStatement;
}

/** `implements Foo<T>` clause member. */
export interface TSExpressionWithTypeArguments {
	type: 'TSExpressionWithTypeArguments';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
	typeParameters?: TSTypeParameterInstantiation;
}

/** TS constructor parameter property: `constructor(public x: number)`. */
export interface TSParameterProperty {
	type: 'TSParameterProperty';
	start: number;
	end: number;
	loc: SourceLocation;
	accessibility?: string;
	/** Omitted from JSON when false. */
	readonly?: boolean;
	/** Omitted from JSON when false. */
	override?: boolean;
	parameter: Expression;
}

//
// TypeScript AST — patterns (destructuring)
//

export interface ObjectPattern {
	type: 'ObjectPattern';
	start: number;
	end: number;
	loc: SourceLocation;
	properties: ObjectPatternProperty[];
	optional?: boolean;
	typeAnnotation?: TSTypeAnnotation;
	/** Parameter decorators (`@dec { a }: T`) — only in a parameter position. */
	decorators?: Decorator[];
}

export type ObjectPatternProperty = Property | RestElement;

export interface ArrayPattern {
	type: 'ArrayPattern';
	start: number;
	end: number;
	loc: SourceLocation;
	elements: (Expression | null)[];
	optional?: boolean;
	typeAnnotation?: TSTypeAnnotation;
	/** Parameter decorators (`@dec [a]: T`) — only in a parameter position. */
	decorators?: Decorator[];
}

export interface AssignmentPattern {
	type: 'AssignmentPattern';
	start: number;
	end: number;
	loc: SourceLocation;
	left: Expression;
	right: Expression;
	/** Parameter decorators (`@dec a = 1`) — only in a parameter position. */
	decorators?: Decorator[];
}

export interface RestElement {
	type: 'RestElement';
	start: number;
	end: number;
	loc: SourceLocation;
	argument: Expression;
	/** Optional rest parameter (`...a?`) — only in a parameter position (invalid TS, deferred). */
	optional?: boolean;
	typeAnnotation?: TSTypeAnnotation;
}

//
// TypeScript AST — declarations
//

export interface TSInterfaceDeclaration {
	type: 'TSInterfaceDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	id: Identifier;
	typeParameters?: TSTypeParameterDeclaration;
	/** Omitted from JSON when empty. */
	extends?: TSInterfaceHeritage[];
	body: TSInterfaceBody;
	/** Omitted from JSON when false. */
	declare?: boolean;
}

export interface TSInterfaceHeritage {
	type: 'TSInterfaceHeritage';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: TSEntityName;
	typeParameters?: TSTypeParameterInstantiation;
}

/** `declare function foo(): void` or overload signature. */
export interface TSDeclareFunction {
	type: 'TSDeclareFunction';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Omitted from JSON when false. */
	declare?: boolean;
	id: Identifier;
	expression: boolean;
	generator: boolean;
	async: boolean;
	typeParameters?: TSTypeParameterDeclaration;
	params: Expression[];
	returnType?: TSTypeAnnotation;
}

export interface TSEnumDeclaration {
	type: 'TSEnumDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Omitted from JSON when false. */
	const?: boolean;
	/** Omitted from JSON when false. */
	declare?: boolean;
	id: Identifier;
	members: TSEnumMember[];
}

export interface TSEnumMember {
	type: 'TSEnumMember';
	start: number;
	end: number;
	loc: SourceLocation;
	id: TSEnumMemberId;
	initializer?: Expression;
}

export type TSEnumMemberId = Identifier | Literal;

/** `namespace A { ... }` or `module A { ... }`. */
export interface TSModuleDeclaration {
	type: 'TSModuleDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	/** `declare global {}` flag. Omitted from JSON when false. */
	global?: boolean;
	id: TSModuleName;
	/** None for shorthand ambient modules (`declare module 'name';`). */
	body?: TSModuleDeclarationBody;
	/** Omitted from JSON when false. */
	declare?: boolean;
}

export type TSModuleName = Identifier | Literal;

export type TSModuleDeclarationBody = TSModuleBlock | TSModuleDeclaration;

export interface TSModuleBlock {
	type: 'TSModuleBlock';
	start: number;
	end: number;
	loc: SourceLocation;
	body: Statement[];
}

//
// TypeScript AST — imports / exports
//

export interface ExportNamedDeclaration {
	type: 'ExportNamedDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Omitted in Svelte non-`lang="ts"` context when `"value"`. */
	exportKind?: string;
	declaration: Statement | null;
	specifiers: ExportSpecifier[];
	source: Literal | null;
	/** Present in Svelte non-`lang="ts"` context; omitted in TS context when empty. */
	attributes?: ImportAttribute[];
}

export interface ExportDefaultDeclaration {
	type: 'ExportDefaultDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Omitted in Svelte non-`lang="ts"` context. */
	exportKind?: string;
	declaration: ExportDefaultValue;
}

export type ExportDefaultValue =
	| Expression
	| FunctionDeclaration
	| TSDeclareFunction
	| ClassDeclaration
	| TSInterfaceDeclaration;

/**
 * A module export name: a bare `Identifier` or a string `Literal`. Per ecma262
 * `ModuleExportName : IdentifierName | StringLiteral` (ES2022 arbitrary module
 * namespace names) — e.g. `import { 'str' as b }`, `export { x as 'str' }`,
 * `export * as 'str' from`.
 */
export type ModuleExportName = Identifier | Literal;

export interface ExportAllDeclaration {
	type: 'ExportAllDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Omitted in Svelte non-`lang="ts"` context when `"value"`. */
	exportKind?: string;
	exported: ModuleExportName | null;
	source: Literal;
	/** Present in Svelte non-`lang="ts"` context; omitted in TS context when empty. */
	attributes?: ImportAttribute[];
}

/** `export = value;`. */
export interface TSExportAssignment {
	type: 'TSExportAssignment';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Expression;
}

/** `export as namespace Foo;` — TypeScript UMD global export. */
export interface TSNamespaceExportDeclaration {
	type: 'TSNamespaceExportDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	id: Identifier;
}

export interface ExportSpecifier {
	type: 'ExportSpecifier';
	start: number;
	end: number;
	loc: SourceLocation;
	local: ModuleExportName;
	exported: ModuleExportName;
	/** Omitted in Svelte non-`lang="ts"` context when `"value"`. */
	exportKind?: string;
}

export interface ImportDeclaration {
	type: 'ImportDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Omitted in Svelte non-`lang="ts"` context when `"value"`. */
	importKind?: string;
	/** Import phase (`'source'`/`'defer'`) for `import source …` / `import defer …`; omitted otherwise. */
	phase?: 'source' | 'defer';
	specifiers: ImportSpecifier[];
	source: Literal;
	/** Present in Svelte non-`lang="ts"` context (even when empty); omitted in TS context when empty. */
	attributes?: ImportAttribute[];
}

export type ImportSpecifier =
	| ImportDefaultSpecifier
	| ImportNamedSpecifier
	| ImportNamespaceSpecifier;

export interface ImportDefaultSpecifier {
	type: 'ImportDefaultSpecifier';
	start: number;
	end: number;
	loc: SourceLocation;
	local: Identifier;
}

export interface ImportNamedSpecifier {
	/** Acorn names named imports `"ImportSpecifier"` (no `Named` prefix). */
	type: 'ImportSpecifier';
	start: number;
	end: number;
	loc: SourceLocation;
	imported: ModuleExportName;
	local: Identifier;
	/** Omitted in Svelte non-`lang="ts"` context when `"value"`. */
	importKind?: string;
}

export interface ImportNamespaceSpecifier {
	type: 'ImportNamespaceSpecifier';
	start: number;
	end: number;
	loc: SourceLocation;
	local: Identifier;
}

export interface ImportAttribute {
	type: 'ImportAttribute';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Bare `type` → `Identifier`; quoted `'resolution-mode'` → `Literal`. */
	key: Identifier | Literal;
	value: Literal;
}

/** `import x = require("y")` or `import x = A.B`. */
export interface TSImportEqualsDeclaration {
	type: 'TSImportEqualsDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	importKind: string;
	isExport: boolean;
	id: Identifier;
	moduleReference: TSModuleReference;
}

export type TSModuleReference = TSExternalModuleReference | TSEntityName;

export interface TSExternalModuleReference {
	type: 'TSExternalModuleReference';
	start: number;
	end: number;
	loc: SourceLocation;
	expression: Literal;
}

//
// TypeScript AST — types
//

export interface TSTypeAnnotation {
	type: 'TSTypeAnnotation';
	start: number;
	end: number;
	loc: SourceLocation;
	typeAnnotation: TSType;
}

export type TSType =
	| TSNumberKeyword
	| TSStringKeyword
	| TSBooleanKeyword
	| TSAnyKeyword
	| TSVoidKeyword
	| TSUndefinedKeyword
	| TSNullKeyword
	| TSNeverKeyword
	| TSUnknownKeyword
	| TSObjectKeyword
	| TSSymbolKeyword
	| TSBigIntKeyword
	| TSLiteralType
	| TSArrayType
	| TSUnionType
	| TSIntersectionType
	| TSTypeReference
	| TSTypeLiteral
	| TSFunctionType
	| TSConstructorType
	| TSTupleType
	| TSParenthesizedType
	| TSTypePredicate
	| TSConditionalType
	| TSMappedType
	| TSTypeOperator
	| TSImportType
	| TSTypeQuery
	| TSIndexedAccessType
	| TSRestType
	| TSOptionalType
	| TSNamedTupleMember
	| TSInferType
	| TSThisType;

export interface TSArrayType {
	type: 'TSArrayType';
	start: number;
	end: number;
	loc: SourceLocation;
	elementType: TSType;
}

/** `T[K]`, `Obj["key"]`. */
export interface TSIndexedAccessType {
	type: 'TSIndexedAccessType';
	start: number;
	end: number;
	loc: SourceLocation;
	objectType: TSType;
	indexType: TSType;
}

export interface TSNumberKeyword {
	type: 'TSNumberKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSStringKeyword {
	type: 'TSStringKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSBooleanKeyword {
	type: 'TSBooleanKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSAnyKeyword {
	type: 'TSAnyKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSVoidKeyword {
	type: 'TSVoidKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSUndefinedKeyword {
	type: 'TSUndefinedKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSNullKeyword {
	type: 'TSNullKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSNeverKeyword {
	type: 'TSNeverKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSUnknownKeyword {
	type: 'TSUnknownKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSObjectKeyword {
	type: 'TSObjectKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSSymbolKeyword {
	type: 'TSSymbolKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSBigIntKeyword {
	type: 'TSBigIntKeyword';
	start: number;
	end: number;
	loc: SourceLocation;
}

/** `type X = T`. */
export interface TSTypeAliasDeclaration {
	type: 'TSTypeAliasDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	id: Identifier;
	typeParameters?: TSTypeParameterDeclaration;
	typeAnnotation: TSType;
	/** Omitted from JSON when false. */
	declare?: boolean;
}

/** `type X = 'hello'` or `type X = \`template\``. */
export interface TSLiteralType {
	type: 'TSLiteralType';
	start: number;
	end: number;
	loc: SourceLocation;
	literal: TSLiteralTypeLiteral;
}

export type TSLiteralTypeLiteral = TemplateLiteralType | UnaryExpression | Literal;

/** Template literal in type position (same structure as `TemplateLiteral` but expressions are `TSType`). */
export interface TemplateLiteralType {
	type: 'TemplateLiteral';
	start: number;
	end: number;
	loc: SourceLocation;
	expressions: TSType[];
	quasis: TemplateElement[];
}

/** `Foo` or `Foo.Bar.Baz`. */
export type TSEntityName = Identifier | TSQualifiedName;

export interface TSQualifiedName {
	type: 'TSQualifiedName';
	start: number;
	end: number;
	loc: SourceLocation;
	left: TSEntityName;
	right: Identifier;
}

/** `<T, U>` in call/instantiation position. */
export interface TSTypeParameterInstantiation {
	type: 'TSTypeParameterInstantiation';
	start: number;
	end: number;
	loc: SourceLocation;
	params: TSType[];
}

/** `<T extends U = V>` in declaration position. */
export interface TSTypeParameterDeclaration {
	type: 'TSTypeParameterDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	params: TSTypeParameter[];
	extra?: TSTypeParameterExtra;
}

export interface TSTypeParameterExtra {
	trailingComma: number;
}

/** Single type parameter with optional modifiers (`const`, `in`, `out`). */
export interface TSTypeParameter {
	type: 'TSTypeParameter';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Omitted from JSON when false. */
	const?: boolean;
	/** Omitted from JSON when false. */
	in?: boolean;
	/** Omitted from JSON when false. */
	out?: boolean;
	name: string;
	constraint?: TSType;
	default?: TSType;
}

export type TSTypeElement =
	| TSPropertySignature
	| TSMethodSignature
	| TSCallSignatureDeclaration
	| TSConstructSignatureDeclaration
	| TSIndexSignature;

export interface TSInterfaceBody {
	type: 'TSInterfaceBody';
	start: number;
	end: number;
	loc: SourceLocation;
	body: TSTypeElement[];
}

/** `prop: T`, `readonly prop?: T`. */
export interface TSPropertySignature {
	type: 'TSPropertySignature';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Omitted from JSON when false. */
	readonly?: boolean;
	/** acorn omits this when `key` is the `new` keyword. */
	computed?: boolean;
	key: Expression;
	/** Omitted from JSON when false. */
	optional?: boolean;
	typeAnnotation?: TSTypeAnnotation;
}

/** `method(): T`, `get prop(): T`, `set prop(v: T)`. */
export interface TSMethodSignature {
	type: 'TSMethodSignature';
	start: number;
	end: number;
	loc: SourceLocation;
	computed: boolean;
	key: Expression;
	/** Omitted from JSON when false. */
	optional?: boolean;
	/** `"get"` or `"set"` for accessor signatures; omitted for regular methods. */
	kind?: string;
	typeParameters?: TSTypeParameterDeclaration;
	parameters: Expression[];
	/** Return type. Serialized as `typeAnnotation` to match acorn-typescript. */
	typeAnnotation?: TSTypeAnnotation;
}

/** `(): T`. */
export interface TSCallSignatureDeclaration {
	type: 'TSCallSignatureDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	typeParameters?: TSTypeParameterDeclaration;
	parameters: Expression[];
	typeAnnotation?: TSTypeAnnotation;
}

/** `new (): T`. */
export interface TSConstructSignatureDeclaration {
	type: 'TSConstructSignatureDeclaration';
	start: number;
	end: number;
	loc: SourceLocation;
	typeParameters?: TSTypeParameterDeclaration;
	parameters: Expression[];
	typeAnnotation?: TSTypeAnnotation;
}

/** `[key: string]: T`. */
export interface TSIndexSignature {
	type: 'TSIndexSignature';
	start: number;
	end: number;
	loc: SourceLocation;
	/** Omitted from JSON when false. */
	static?: boolean;
	/** Omitted from JSON when false. */
	readonly?: boolean;
	parameters: Identifier[];
	/** Omitted from JSON for a typeless index signature (`[key: string]`). */
	typeAnnotation?: TSTypeAnnotation;
}

export interface TSUnionType {
	type: 'TSUnionType';
	start: number;
	end: number;
	loc: SourceLocation;
	types: TSType[];
}

export interface TSIntersectionType {
	type: 'TSIntersectionType';
	start: number;
	end: number;
	loc: SourceLocation;
	types: TSType[];
}

export interface TSTypeReference {
	type: 'TSTypeReference';
	start: number;
	end: number;
	loc: SourceLocation;
	typeName: TSEntityName;
	typeArguments?: TSTypeParameterInstantiation;
}

export interface TSTypeLiteral {
	type: 'TSTypeLiteral';
	start: number;
	end: number;
	loc: SourceLocation;
	members: TSTypeElement[];
}

/** `(x: T) => U`, `<T>(x: T) => U`. */
export interface TSFunctionType {
	type: 'TSFunctionType';
	start: number;
	end: number;
	loc: SourceLocation;
	typeParameters?: TSTypeParameterDeclaration;
	parameters: Expression[];
	typeAnnotation: TSTypeAnnotation;
}

/** `new () => T`, `abstract new <T>() => T`. */
export interface TSConstructorType {
	type: 'TSConstructorType';
	start: number;
	end: number;
	loc: SourceLocation;
	abstract: boolean;
	typeParameters?: TSTypeParameterDeclaration;
	parameters: Expression[];
	typeAnnotation: TSTypeAnnotation;
}

/** `[T, U, V]`. */
export interface TSTupleType {
	type: 'TSTupleType';
	start: number;
	end: number;
	loc: SourceLocation;
	elementTypes: TSType[];
}

/** Rest type in tuples: `...T`. */
export interface TSRestType {
	type: 'TSRestType';
	start: number;
	end: number;
	loc: SourceLocation;
	typeAnnotation: TSType;
}

/** Optional type in tuples: `T?`. */
export interface TSOptionalType {
	type: 'TSOptionalType';
	start: number;
	end: number;
	loc: SourceLocation;
	typeAnnotation: TSType;
}

/** `label: T` or `label?: T` in tuple types. */
export interface TSNamedTupleMember {
	type: 'TSNamedTupleMember';
	start: number;
	end: number;
	loc: SourceLocation;
	optional: boolean;
	label: Identifier;
	elementType: TSType;
}

/** `infer U` in conditional types. */
export interface TSInferType {
	type: 'TSInferType';
	start: number;
	end: number;
	loc: SourceLocation;
	typeParameter: TSTypeParameter;
}

/** `this` in type position. */
export interface TSThisType {
	type: 'TSThisType';
	start: number;
	end: number;
	loc: SourceLocation;
}

export interface TSParenthesizedType {
	type: 'TSParenthesizedType';
	start: number;
	end: number;
	loc: SourceLocation;
	typeAnnotation: TSType;
}

/** `x is T` or `asserts x is T`. */
export interface TSTypePredicate {
	type: 'TSTypePredicate';
	start: number;
	end: number;
	loc: SourceLocation;
	parameterName: TSTypePredicateParameterName;
	typeAnnotation: TSTypeAnnotation | null;
	asserts: boolean;
}

export type TSTypePredicateParameterName = Identifier | TSThisType;

/** `T extends U ? V : W`. */
export interface TSConditionalType {
	type: 'TSConditionalType';
	start: number;
	end: number;
	loc: SourceLocation;
	checkType: TSType;
	extendsType: TSType;
	trueType: TSType;
	falseType: TSType;
}

/** `{ [K in keyof T]: V }`. */
export interface TSMappedType {
	type: 'TSMappedType';
	start: number;
	end: number;
	loc: SourceLocation;
	readonly?: TSMappedTypeModifier;
	typeParameter: TSMappedTypeParameter;
	/** Key remapping: `as NewK`. */
	nameType: TSType | null;
	optional?: TSMappedTypeModifier;
	/** The value type — omitted entirely when absent (`{ [K in T] }`). */
	typeAnnotation?: TSType;
}

export interface TSMappedTypeParameter {
	type: 'TSMappedTypeParameter';
	start: number;
	end: number;
	loc: SourceLocation;
	name: string;
	constraint?: TSType;
}

/** Mapped-type modifier: `true`, `"+"`, or `"-"`. */
export type TSMappedTypeModifier = true | '+' | '-';

/** `keyof T`, `unique symbol`, `readonly T`. */
export interface TSTypeOperator {
	type: 'TSTypeOperator';
	start: number;
	end: number;
	loc: SourceLocation;
	operator: string;
	typeAnnotation: TSType;
}

/** `import('module')`, `import('module', {with: {...}}).Qualifier<T>`. */
export interface TSImportType {
	type: 'TSImportType';
	start: number;
	end: number;
	loc: SourceLocation;
	argument: Literal;
	options?: Expression;
	qualifier?: TSEntityName;
	typeArguments?: TSTypeParameterInstantiation;
}

export type TSTypeQueryExprName = Identifier | TSQualifiedName | TSImportType;

/** `typeof x`, `typeof Foo.bar`, `typeof import("module")`, `typeof Array<T>`. */
export interface TSTypeQuery {
	type: 'TSTypeQuery';
	start: number;
	end: number;
	loc: SourceLocation;
	exprName: TSTypeQueryExprName;
	typeArguments?: TSTypeParameterInstantiation;
}

//
// CSS AST
//

/**
 * Parsed CSS — content of a `<style>` tag in a Svelte component.
 *
 * Children and attributes are emitted as opaque JSON values that follow
 * Svelte's `parseCss` output; their precise shape is not currently
 * mirrored in this file.
 */
export interface StyleSheet {
	type: 'StyleSheet';
	start: number;
	end: number;
	attributes: unknown[];
	children: unknown[];
	content: StyleContent;
}

/**
 * Standalone CSS file parse — what `parse_css` returns when CSS is not
 * embedded in a Svelte component. Matches Svelte's `parseCss(...)` output
 * shape: only `children` (no `attributes`/`content`).
 */
export interface StyleSheetFile {
	type: 'StyleSheetFile';
	start: number;
	end: number;
	children: unknown[];
}

/** Raw CSS text inside a `<style>` block. */
export interface StyleContent {
	start: number;
	end: number;
	styles: string;
	comment: unknown | null;
}

//
// Svelte AST
//

/** Span of a name (element, attribute, directive), with byte offsets. */
export interface NameLocation {
	start: NamePosition;
	end: NamePosition;
}

export interface NamePosition {
	line: number;
	column: number;
	character: number;
}

/** Root node of a `.svelte` file. */
export interface Root {
	css: StyleSheet | null;
	js: unknown[];
	start: number;
	end: number;
	type: 'Root';
	fragment: Fragment;
	options: SvelteOptions | null;
	comments: unknown[];
	instance?: Script;
	module?: Script;
}

export interface Fragment {
	type: 'Fragment';
	nodes: FragmentNode[];
}

export type FragmentNode =
	| Element
	| SpecialElement
	| ExpressionTag
	| Text
	| Comment
	| IfBlock
	| EachBlock
	| AwaitBlock
	| KeyBlock
	| SnippetBlock
	| HtmlTag
	| ConstTag
	| DeclarationTag
	| DebugTag
	| RenderTag;

/** HTML comment in template: `<!-- content -->`. */
export interface Comment {
	type: 'Comment';
	start: number;
	end: number;
	data: string;
}

/** HTML element or component tag (the `type` field distinguishes them at runtime). */
export interface Element {
	type: 'Component' | 'RegularElement';
	start: number;
	end: number;
	name: string;
	name_loc: NameLocation;
	attributes: AttributeNode[];
	fragment: Fragment;
}

/**
 * Special Svelte elements: `<svelte:head>`, `<svelte:window>`,
 * `<svelte:body>`, `<svelte:document>`, `<svelte:element>`,
 * `<svelte:component>`, `<svelte:self>`, `<slot>`, `<svelte:fragment>`,
 * `<svelte:boundary>`, and `<title>` inside `<svelte:head>`.
 */
export interface SpecialElement {
	type: string;
	start: number;
	end: number;
	name: string;
	name_loc: NameLocation;
	attributes: AttributeNode[];
	fragment: Fragment;
	/** Dynamic tag for `<svelte:element this={tag}>`. */
	tag?: unknown;
	/** Component expression for `<svelte:component this={Component}>`. */
	expression?: Expression;
}

/** `<svelte:options runes={true} />`. */
export interface SvelteOptions {
	start: number;
	end: number;
	attributes: AttributeNode[];
	runes?: boolean;
	immutable?: boolean;
	accessors?: boolean;
	preserveWhitespace?: boolean;
	css?: string;
	namespace?: string;
	customElement?: unknown;
}

/** Regular `name=value` element attribute. */
export interface Attribute {
	type: 'Attribute';
	start: number;
	end: number;
	name: string;
	name_loc: NameLocation;
	value?: unknown;
}

/** `{@attach ...}` (Svelte 5.29+). */
export interface AttachTag {
	type: 'AttachTag';
	start: number;
	end: number;
	expression: Expression;
}

/** `{...obj}` spread attribute. */
export interface SpreadAttribute {
	type: 'SpreadAttribute';
	start: number;
	end: number;
	expression: Expression;
}

/** `on:click={handler}`. */
export interface OnDirective {
	start: number;
	end: number;
	type: 'OnDirective';
	name: string;
	name_loc: NameLocation;
	expression: Expression | null;
	modifiers: string[];
}

/** `bind:value={name}`. */
export interface BindDirective {
	start: number;
	end: number;
	type: 'BindDirective';
	name: string;
	name_loc: NameLocation;
	/** Shorthand and explicit forms produce different field orderings; emitted as raw JSON. */
	expression: unknown;
	modifiers: string[];
}

/** `class:active={cond}`. */
export interface ClassDirective {
	start: number;
	end: number;
	type: 'ClassDirective';
	name: string;
	name_loc: NameLocation;
	expression: unknown;
	modifiers: string[];
}

/** `style:color={value}`. */
export interface StyleDirective {
	start: number;
	end: number;
	type: 'StyleDirective';
	name: string;
	name_loc: NameLocation;
	modifiers: string[];
	/** `true | ExpressionTag | (Text | ExpressionTag)[]`. */
	value: unknown;
}

/** `use:action={params}`. */
export interface UseDirective {
	start: number;
	end: number;
	type: 'UseDirective';
	name: string;
	name_loc: NameLocation;
	expression: Expression | null;
	modifiers: string[];
}

/** `transition:fade`, `in:fly`, `out:slide`. */
export interface TransitionDirective {
	start: number;
	end: number;
	type: 'TransitionDirective';
	name: string;
	name_loc: NameLocation;
	expression: Expression | null;
	modifiers: string[];
	intro: boolean;
	outro: boolean;
}

/** `animate:flip={params}`. */
export interface AnimateDirective {
	start: number;
	end: number;
	type: 'AnimateDirective';
	name: string;
	name_loc: NameLocation;
	expression: Expression | null;
	modifiers: string[];
}

/** `let:item={localItem}` slot prop. */
export interface LetDirective {
	start: number;
	end: number;
	type: 'LetDirective';
	name: string;
	name_loc: NameLocation;
	expression: Expression | null;
	modifiers: string[];
}

export type AttributeNode =
	| Attribute
	| SpreadAttribute
	| AttachTag
	| OnDirective
	| BindDirective
	| ClassDirective
	| StyleDirective
	| UseDirective
	| TransitionDirective
	| AnimateDirective
	| LetDirective;

export type AttributeValue = AttributeText | ExpressionTag;

/**
 * Text inside an attribute value.
 *
 * Note: attribute-value `Text` nodes serialize `start`/`end` before `type`,
 * unlike fragment-level `Text` nodes.
 */
export interface AttributeText {
	start: number;
	end: number;
	type: 'Text';
	raw: string;
	data: string;
}

/** Text node in a fragment. */
export interface Text {
	type: 'Text';
	start: number;
	end: number;
	raw: string;
	data: string;
}

/** `{expression}` in template position. */
export interface ExpressionTag {
	type: 'ExpressionTag';
	start: number;
	end: number;
	expression: Expression;
}

/**
 * Svelte `<script>` block.
 *
 * `content` is a `Program`-shaped JSON value with `leadingComments` /
 * `trailingComments` injected onto its nodes; the exact comment shape is
 * not currently mirrored in this file.
 */
export interface Script {
	type: 'Script';
	start: number;
	end: number;
	/** `"default"` or `"module"`. */
	context: string;
	content: unknown;
	attributes: AttributeNode[];
}

/** `{#if test}...{/if}` and `{:else if test}...` chains. */
export interface IfBlock {
	type: 'IfBlock';
	elseif: boolean;
	start: number;
	end: number;
	test: Expression;
	consequent: Fragment;
	alternate: Fragment | null;
}

/** `{#each expr as item, i (key)}...{/each}`. */
export interface EachBlock {
	type: 'EachBlock';
	start: number;
	end: number;
	expression: Expression;
	body: Fragment;
	/** `null` when no `as` clause. */
	context: unknown;
	index?: string;
	key?: Expression;
	fallback?: Fragment;
}

/** `{#await promise}...{:then value}...{:catch error}...{/await}`. */
export interface AwaitBlock {
	type: 'AwaitBlock';
	start: number;
	end: number;
	expression: Expression;
	value: unknown;
	error: unknown;
	pending: Fragment | null;
	then: Fragment | null;
	catch: Fragment | null;
}

/** `{#key expression}...{/key}`. */
export interface KeyBlock {
	type: 'KeyBlock';
	start: number;
	end: number;
	expression: Expression;
	fragment: Fragment;
}

/** `{#snippet name(params)}...{/snippet}`. */
export interface SnippetBlock {
	type: 'SnippetBlock';
	start: number;
	end: number;
	expression: Expression;
	parameters: Expression[];
	body: Fragment;
	typeParams?: string;
}

/** `{@html expression}`. */
export interface HtmlTag {
	type: 'HtmlTag';
	start: number;
	end: number;
	expression: Expression;
}

/**
 * `{@const x = expr}`.
 *
 * `declaration` is a `VariableDeclaration`-shaped value with a single
 * declarator, emitted as raw JSON.
 */
export interface ConstTag {
	type: 'ConstTag';
	start: number;
	end: number;
	declaration: unknown;
}

/**
 * `{const x = expr}` / `{let x = expr}` / `{let x}` — the bare declaration tags
 * (no `@`).
 *
 * `declaration` is a `VariableDeclaration`-shaped value (`kind` is `const` or
 * `let`) with a single declarator, emitted as raw JSON.
 */
export interface DeclarationTag {
	type: 'DeclarationTag';
	start: number;
	end: number;
	declaration: unknown;
}

/** `{@debug a, b, c}`. */
export interface DebugTag {
	type: 'DebugTag';
	start: number;
	end: number;
	identifiers: Expression[];
}

/** `{@render snippet(args)}`. */
export interface RenderTag {
	type: 'RenderTag';
	start: number;
	end: number;
	expression: Expression;
}

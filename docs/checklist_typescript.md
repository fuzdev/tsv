# TypeScript Language Support

Comprehensive reference for TypeScript/JS language features supported by tsv's parser and formatter.

## Coverage

Every syntax feature of the published ECMAScript standard is supported, as enumerated below — through **ES2025**, the most recent edition that added grammar (import attributes; its two RegExp additions ride the opaque regex body, see [Regular Expressions](#regular-expressions)). ES2026 added library APIs only, no new syntax. The finished (Stage 4) `using` declarations, awaiting publication in a later edition, are supported too — see [Explicit Resource Management](#explicit-resource-management). On the TypeScript side every construct the `tsc` oracle parses is parsed here, down to its newest contextual keyword (`defer`); the `conformance:ts-fixtures` and `conformance:ts-repo` gates pin those oracles' versions, so this doc names none.

Sloppy-mode constructs are excluded by design (see [Out of Scope](#out-of-scope)). ECMAScript conformance is measured against test262 (see [conformance_test262.md](./conformance_test262.md)). Constructs tsv does not parse — all of them TC39 proposals — are listed under [Future Work](#future-work).

**On proposal maturity**: this doc deliberately carries **no TC39 stage labels**. A stage can change at any TC39 meeting, and no gate here can catch a label that has gone stale — so a stage quoted in this file would be a claim nothing keeps honest. Year tags on shipped features (`- ES2020`) are a different thing: an edition is a historical fact and does not rot, so those stay. For a construct's current stage, read the oracle — `../../proposals/`, where `finished-proposals.md` is Stage 4, `README.md` is Stage 2 and up, and the rest are `stage-1-proposals.md`, `stage-0-proposals.md`, `inactive-proposals.md`.

**Spec References**:

- ECMAScript spec: `../../ecma262/spec.html` (see [CLAUDE.md](../../ecma262/CLAUDE.md))
- Test262 suite: `../../test262/test/` (see [CLAUDE.md](../../test262/CLAUDE.md))
- Test262 features: `../../test262/features.txt` (the upstream feature-flag list)

---

# Supported Features

## Lexical Grammar

Foundation for all parsing.

### Tokens & Whitespace

- Whitespace tokens — ECMAScript WhiteSpace (space, tab, VT, FF, NBSP, ZWNBSP/U+FEFF, other Zs)
- Line terminators (LF, CR, CRLF, LS/U+2028, PS/U+2029)
- Unicode BOM handling (intentional divergence: tsv strips BOM)
- Hashbang comments (`#!/usr/bin/env node`, terminated by any line terminator) - ES2023

### Comments

- Line comments (`// comment`)
- Block comments (`/* comment */`)
- JSDoc comments (`/** @param */`)
- Nested comment handling
- Comment preservation in AST
- Leading/trailing comment attachment
- `format-ignore` / `prettier-ignore` directive (`// format-ignore` emits the next construct verbatim — see [directives.md](./directives.md))

### Identifiers

- ASCII identifiers (`foo`, `_private`, `$jquery`)
- Unicode identifiers (`π`, `日本語`) — full `ID_Start`/`ID_Continue` per UAX #31, including the `Other_ID_*` and NFKC-excluded code points (`゛` U+309B, …) that `XID_*` drops
- Escaped identifiers (`\u0041` = A)
- Private identifiers (`#privateField`) - ES2022
- Reserved word restrictions

---

## Literals

### Numeric Literals

- Decimal integers (`42`, `0`)
- Decimal floats (`3.14`, `.5`, `5.`)
- Exponential notation (`1e10`, `1E-5`)
- Hexadecimal (`0xFF`, `0XFF`)
- Octal (`0o77`, `0O77`)
- Binary (`0b1010`, `0B1010`)
- Numeric separators (`1_000_000`) - ES2021
- BigInt literals (`123n`, `0xFFn`)

### String Literals

- Single quotes (`'text'`)
- Double quotes (`"text"`)
- Escape sequences (`\n`, `\t`, `\\`, `\'`, `\"`)
- Hex escapes (`\x41`)
- Unicode 4-digit (`\u0041`)
- Unicode codepoint (`\u{1F600}`)
- Line continuation (`\` at EOL)
- Null character (`\0`)

### Template Literals

- Basic templates (`` `text` ``)
- Substitutions (`` `${expr}` ``)
- Nested templates
- Tagged templates (`` tag`text` ``)
- Raw strings (`String.raw`)

### Boolean & Null

- `true`, `false`
- `null`
- `undefined` (identifier, not literal)

### Regular Expressions

The pattern body and the flag run are **opaque** — carried verbatim as source slices, never
parsed. This is what the spec asks a lexer for, not a shortcut: the `RegularExpressionBody` /
`RegularExpressionFlags` productions exist so "the input element scanner [can] find the end of
the regular expression literal", and the text they cover is "subsequently parsed again using
the more stringent ECMAScript Regular Expression grammar"
(ecma262 §sec-literals-regular-expression-literals). So the lexer scans only far enough to find
the literal's end — tracking `\` escapes and `[…]` class nesting so a `/` inside either doesn't
terminate early, and rejecting an unterminated or empty literal. Everything below therefore
round-trips, at every flag and every pattern grammar, including ones newer than this list:

- Basic patterns (`/pattern/`)
- Flags: `d` (indices), `g`, `i`, `m`, `s`, `u`, `v` (unicodeSets), `y`
- Character classes (`[a-z]`, `\d`, `\w`), escapes in patterns
- Division vs regex disambiguation

That second, stringent parse is an **early error** — "It is a Syntax Error if
`IsValidRegularExpressionLiteral(RegularExpressionLiteral)` is false" — which places it with
the other early errors tsv defers (see [Out of Scope](#out-of-scope)). So an invalid flag
(`/a/qqq`), a duplicate flag (`/a/gg`), and an invalid pattern (`/(?zz:a)/`, `/a{2,1}/u`) all
parse here while acorn rejects them; prettier's parser is likewise regex-opaque and formats all
four. The formatter re-emits the body verbatim and never needs the pattern grammar, so the
whole check is one self-contained spec operation for the diagnostics layer to run: flags ⊆
`{d,g,i,m,s,u,v,y}` with no repeats, then `ParsePattern(patternText, u, v)`.

What the lexer *does* enforce is the flags production exactly — `IdentifierPartChar`, i.e.
`UnicodeIDContinue` or `$`. That admits no backslash, so a unicode-escaped flag (a `\u` escape
written in the flags position) is not a flags production at all and is rejected, matching acorn.

---

## Expressions

### Primary Expressions

- `this` keyword
- Identifier references
- Literal expressions
- Array literals (`[1, 2, 3]`)
- Object literals (`{a: 1, b: 2}`)
- Parenthesized expressions (`(expr)`)
- Regular expression literals

### Array Literals

- Empty arrays (`[]`)
- Element list (`[1, 2, 3]`)
- Trailing commas (`[1, 2,]`)
- Sparse arrays / elisions (`[1, , 3]`)
- Spread elements (`[...arr]`)

### Object Literals

- Property definitions (`{a: 1}`)
- Shorthand properties (`{x, y}`)
- Computed properties (`{[expr]: value}`)
- Spread properties (`{...obj}`) - ES2018
- Method definitions (`{method() {}}`)
- Getter definitions (`{get x() {}}`)
- Setter definitions (`{set x(v) {}}`)
- Async methods (`{async method() {}}`)
- Generator methods (`{*gen() {}}`)
- `__proto__` property (Annex B)

### Member Expressions

- Dot notation (`obj.prop`)
- Bracket notation (`obj["prop"]`)
- Computed access (`obj[expr]`)
- Chained access (`a.b.c`)
- Private field access (`obj.#field`)
- Optional chaining (`obj?.prop`) - ES2020
- Optional bracket (`obj?.[key]`) - ES2020

### Call Expressions

- Function calls (`fn()`)
- Method calls (`obj.method()`)
- Chained calls (`a().b()`)
- Arguments list
- Spread arguments (`fn(...args)`)
- Optional call (`fn?.()`) - ES2020
- Tagged template calls (`` tag`str` ``)

### New Expressions

- `new Constructor()`
- `new Constructor` (no parens)
- Chained new (`new a.b()`)
- `new.target` meta-property

### Super

- `super()` in constructors
- `super.method()` in methods
- `super[prop]` computed access

### Unary Operators

- `delete expr`
- `void expr`
- `typeof expr`
- Unary `+` and `-`
- Bitwise NOT `~`
- Logical NOT `!`
- `await expr` (in async)
- `yield expr` (in generators)
- `yield* expr` (delegation)
- Operand parenthesization preserved where dropping parens would merge tokens — a same-sign `+`/`-` operand stays wrapped (`+(+x)` not `++x`, `-(--x)` not `---x`, `+(++x)` not `+++x`)

### Update Expressions

- Prefix increment (`++x`)
- Prefix decrement (`--x`)
- Postfix increment (`x++`)
- Postfix decrement (`x--`)
- Operand parenthesization preserved for a type-assertion operand — `(a as T)++` not `a as T++`

### Binary Operators

- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Exponentiation: `**` - ES2016
- Comparison: `<`, `>`, `<=`, `>=`
- Equality: `==`, `!=`, `===`, `!==`
- Bitwise: `&`, `|`, `^`, `<<`, `>>`, `>>>`
- Logical: `&&`, `||`
- Nullish coalescing: `??` - ES2020
- `in` operator
- `instanceof` operator

### Assignment Operators

- Simple assignment: `=`
- Compound: `+=`, `-=`, `*=`, `/=`, `%=`, `**=`
- Compound: `&=`, `|=`, `^=`, `<<=`, `>>=`, `>>>=`
- Logical assignment: `&&=`, `||=`, `??=` - ES2021
- Destructuring assignment

### Conditional Operator

- Ternary (`a ? b : c`)
- Nested ternaries
- Operator precedence

### Comma Operator

- Sequence expressions (`a, b, c`)

### Arrow Functions

- Expression body (`x => x + 1`)
- Block body (`x => { return x; }`)
- Single parameter (`x => x`)
- Multiple parameters (`(a, b) => a + b`)
- No parameters (`() => expr`)
- Rest parameters (`(...args) => {}`)
- Default parameters (`(x = 1) => x`)
- Destructured parameters
- Async arrows (`async () => {}`)

### Function Expressions

- Anonymous (`function() {}`)
- Named (`function name() {}`)
- Generator (`function*() {}`)
- Async (`async function() {}`)
- Async generator (`async function*() {}`)

### Class Expressions

- Anonymous (`class {}`)
- Named (`class Name {}`)
- With extends (`class extends Base {}`)

### Dynamic Import

- `import()` expression - ES2020
- `import.meta` meta-property
- Phased dynamic import (`import.source(…)` / `import.defer(…)`) — the Source Phase Imports / Deferring Module Evaluation proposals, not yet standard; tsv-native, acorn rejects (see [conformance_svelte.md](./conformance_svelte.md#import-phase-proposals))

---

## Statements

### Block & Empty

- Block statement (`{ }`)
- Empty statement (`;`)
- Expression statement

### Control Flow

- `if` statement
- `if...else`
- `else if` chains
- `switch` statement
- `case` clauses
- `default` clause
- Fall-through handling

### Loops

- `while` loop
- `do...while` loop
- `for` loop
- `for...in` loop
- `for...of` loop - ES2015
- `for await...of` loop - ES2018

### Jump Statements

- `break` statement
- `break` with label
- `continue` statement
- `continue` with label
- `return` statement
- `throw` statement

### Labeled Statements

- Labels on statements
- Labels on blocks

### Try/Catch/Finally

- `try...catch`
- `try...finally`
- `try...catch...finally`
- Catch binding (`catch (e)`)
- Optional catch binding (`catch { }`) - ES2019

### Debugger

- `debugger` statement

### Automatic Semicolon Insertion (ASI)

- Before `}`
- Before EOF
- After line terminator
- Restricted productions (`return`, `throw`, `break`, `continue`)
- Postfix `++`/`--` restrictions (no line terminator before; and no subscript after — an update expression is not a `LeftHandSideExpression`, so `a++[b]`/`a++.c`/`a++()` are rejected)
- Arrow function handling
- Hazardous patterns (`[`, `(`, `/`, `+`, `-`)

---

## Declarations

### Variable Declarations

- `var` declarations
- `let` declarations - ES2015
- `const` declarations - ES2015
- Multiple declarators (`let a, b, c`)
- Initializers (`let x = 1`)
- Destructuring patterns

### Destructuring Patterns

- Object patterns (`const {a, b} = obj`)
- Array patterns (`const [a, b] = arr`)
- Nested patterns
- Default values (`{a = 1} = {}`)
- Renaming (`{a: b} = obj`)
- Rest patterns (`{...rest}`, `[...rest]`)
- Rest parameter as a binding pattern (`function f(...[a, b]) {}`, `(...{ a }) => {}`)
- Computed properties in patterns

### Function Declarations

- Named functions
- Generator functions (`function*`)
- Async functions (`async function`)
- Async generators (`async function*`)
- Default parameters
- Rest parameters

### Class Declarations

See [Classes](#classes) section for full details.

---

## Modules

ES2015 module syntax with ES2025 additions.

### Import Declarations

- Default import (`import x from 'mod'`)
- Named imports (`import {a, b} from 'mod'`)
- Namespace import (`import * as ns from 'mod'`)
- Combined (`import x, {a} from 'mod'`)
- Renamed imports (`import {a as b} from 'mod'`)
- String import name (`import {'str' as b} from 'mod'`) - ES2022
- Side-effect import (`import 'mod'`)
- Import attributes (`import x from 'y' with {}`) - ES2025
- Phased import (`import source x from 'mod'` / `import defer * as ns from 'mod'`) — the Source Phase Imports / Deferring Module Evaluation proposals, not yet standard; `defer` is TypeScript's newest contextual keyword. tsv-native, acorn rejects (see [conformance_svelte.md](./conformance_svelte.md#import-phase-proposals))

### Export Declarations

- Named exports (`export {a, b}`)
- Renamed exports (`export {a as b}`)
- Declaration exports (`export const x = 1`)
- Default export (`export default expr`)
- Default function (`export default function() {}`)
- Default class (`export default class {}`)
- Default interface (`export default interface Foo {}`)
- Re-exports (`export {a} from 'mod'`)
- String export name (`export {a as 'str'}`, `export {'str'} from 'mod'`) - ES2022
- Re-export all (`export * from 'mod'`)
- Re-export as namespace (`export * as ns from 'mod'`, `export * as 'str' from 'mod'`)

---

## Classes

### Class Structure

- Class declaration
- Class expression
- `extends` clause — any `LeftHandSideExpression` superclass (`extends Base`,
  `extends getMixin(B)`, `extends class {}`, `extends (a + b)`, `extends null`)
- `constructor` method
- Instance methods
- Static methods
- Getter/setter methods
- Computed method names
- Generator methods
- Async methods
- Empty members (stray `;` skipped, matching acorn — prettier strips them)

### Field Declarations (ES2022)

- Public fields
- Private fields (`#field`)
- Static fields
- Static private fields
- Private methods
- Static initialization blocks (`static {}`)
- `#x in obj` (private field check)

### TypeScript Class Features

**Accessibility Modifiers**:

- `public`
- `private`
- `protected`
- `readonly`

**Abstract Classes**:

- `abstract class`
- `abstract` methods
- `abstract` properties

**Class Modifiers**:

- `implements` clause
- Multiple `implements`
- `declare class`

**Decorators** (a TC39 proposal — not in any ECMAScript edition; shipped in TS 5.0):

- Class decorators (`@decorator class C {}`)
- Decorated class expressions (`x = @dec class {}`)
- Method decorators
- Property decorators
- Accessor decorators
- Decorator factories (`@decorator()`)
- Parameter decorators (`fn(@dec x: T)`)
- Decorators on ambient class members (`declare class C { @dec m() {} }`)
- Decorators after `export` (`export @dec class C {}`) — position preserved relative to `export`

Note: Parameter decorators are legacy-TypeScript syntax (not part of the TC39 decorators proposal), but tsv parses them — the parser attaches them to the parameter's `decorators`, covered by `tests/fixtures/typescript/typescript_specific/decorators/parameter/`. They are accepted in exactly the positions acorn's `parseAssignableListItem` reaches: function declarations/expressions, class methods and the constructor, object-literal methods, and ambient `declare function`s. They are **rejected** on arrow-function parameters (`(@dec a) => a`) and in type-member signatures (interface / type-literal method, call, construct, and accessor signatures) — grammar errors acorn, tsc, and prettier all reject — covered by `tests/fixtures/typescript/typescript_specific/decorators/{parameter_arrow,type_member_signature}/`. The lone divergence is `async <T>(@dec a) => a`, which acorn accepts only because of its async-generic param-drop bug; tsv rejects it to match tsc (see `tests/fixtures/typescript/expressions/arrow/async_generic/param_decorator_svelte_divergence/`).

Note: An ambient (`declare class`) member parses decorators exactly like a concrete member — TS's "decorators are not valid here" (TS1206) is a config-dependent early-error (tsc accepts `@dec declare field` under `experimentalDecorators`, rejects it under ES decorators), so the parser accepts structurally and defers the check to the diagnostics layer. Covered by `tests/fixtures/typescript/typescript_specific/declare/class/member_decorators/`.

**Other Features**:

- `override` modifier - TS 4.3
- `accessor` keyword - ES2022/TS 4.9
- Parameter properties (`constructor(public x: T)`) — all modifiers: `public`/`private`/`protected`, `override`, `readonly` (canonical order `accessibility → override → readonly`)

---

## TypeScript Types

### Basic Type Annotations

- Variable annotations (`const x: number = 1`)
- Parameter annotations (`function f(x: number) {}`)
- Return type annotations (`function f(): number {}`)
- Property annotations (`class C { x: number }`)
- Optional properties (`x?: number`)

### Primitive Types

- `any`, `unknown`, `never`, `void`
- `boolean`, `string`, `number`
- `symbol`, `bigint`
- `null`, `undefined`
- `object`

### Literal Types

- String literals (`'value'`)
- Numeric literals (`42`)
- Boolean literals (`true`)
- Template literal types (`` `prefix_${T}` ``)

### Array & Tuple Types

- Array type (`T[]`)
- Postfix array/indexed `[` respects `[no LineTerminator here]` — `T⏎[K]` splits via ASI (`T` then a `[K]` statement), never a `T[K]` suffix (acorn `tsParseArrayTypeOrHigher`)
- Generic array (`Array<T>`)
- Readonly array (`readonly T[]`)
- Tuple types (`[T, U]`)
- Named tuple members (`[name: string]`)
- Optional tuple elements (`[T, U?]`)
- Rest elements in tuples (`[T, ...U[]]`)

### Union & Intersection

- Union types (`A | B`)
- Intersection types (`A & B`)
- Discriminated unions

### Function Types

- Function type (`(x: T) => U`)
- Construct signatures (`new () => T`)
- Call signatures (`{ (): T }`)
- Overloaded signatures

### Object Types

- Object type literals (`{ x: T }`)
- Optional properties (`{ x?: T }`)
- Readonly properties (`{ readonly x: T }`)
- Index signatures (`{ [key: string]: T }`)
- Method signatures (`{ method(): T }`)

### Advanced Type Operators

- `keyof T`
- `typeof x`
- `readonly T`
- `unique symbol`
- Indexed access (`T[K]`)
- Conditional types (`T extends U ? V : W`)
- Mapped types (`{ [K in keyof T]: V }`)
- `infer` in conditionals (incl. constrained `infer U extends C`)

### Type References

- Simple references (`SomeType`)
- Qualified names (`Namespace.Type`)
- Generic instantiation (`Array<T>`)

### Import Types

- Import type (`import('mod').Type`)
- `typeof import('mod')`

---

## TypeScript Syntax

### Type Assertions & Modifiers

- `as` assertion (`expr as Type`)
- Angle-bracket assertion (`<Type>expr`)
- `as const` assertion
- Non-null assertion (`expr!`)
- `satisfies` operator - TS 4.9

### Type Aliases

- `type Foo = T`
- Generic type aliases (`type Foo<T> = ...`)

### Interfaces

- `interface Foo { }`
- `extends` clause
- Multiple `extends`
- Property signatures
- Method signatures
- Index signatures
- Call signatures
- Construct signatures

### Enums

- Numeric enums (`enum E { A, B }`)
- String enums (`enum E { A = 'a' }`)
- Heterogeneous enums
- Computed members
- `const enum`
- `declare enum`

### Ambient Declarations

- `declare const x: T`
- `declare function f(): T`
- `declare class C {}`
- `declare namespace NS {}`
- `declare module 'name' {}`
- `declare global {}`

### Namespaces

- `namespace Foo {}`
- `module Foo {}` (legacy)
- Nested namespaces (`namespace A.B {}`)
- Export from namespaces

### Module Augmentation

- `declare module 'mod' {}`
- `declare global {}`
- Bare `global {}` (no `declare`, top-level or nested in `declare module`)
- Interface merging

### TypeScript-Only Imports/Exports

- `import type { T } from 'mod'`
- `import { type T } from 'mod'`
- `type` name-vs-modifier disambiguation (a two-token lookahead past the `as`,
  matching acorn/tsc): `import { type as age }` is a value import of a binding
  named `type` renamed to `age`; `import { type as as age }` is a type-only
  import of `as` renamed; `import { type as }` a type-only import of `as`;
  bare `import { type }` a value import of `type`. Same rules for `export { … }`.
- A doubled `type` modifier is rejected: `import type { type A }` /
  `export type { type A }` are syntax errors (tsc TS2206 / TS2207), matching
  acorn — the inner `type` disambiguation only runs inside a *value*
  `import { … }` / `export { … }`.
- `export type { T }`
- `export type * from 'mod'`
- `import X = require('mod')`
- `import X = Namespace.Name`
- `import type X = require('mod')` (valid — external module reference); the
  `type` modifier is rejected on an **entity-name** import-equals alias
  (`import type X = A.B` is a syntax error, tsc TS1392). The `export import`
  re-export form mirrors this: `export import type X = require('mod')` is valid,
  `export import type X = A.B` is rejected.
- `export = expr`

### Generics

**Basic Generics**:

- Type parameters (`<T>`)
- Multiple parameters (`<T, U>`)
- Constraints (`<T extends Base>`)
- Default types (`<T = string>`)

**Generic Contexts**:

- Generic functions
- Generic arrow functions (`<T>(x: T) => T`)
- Generic classes
- Generic interfaces
- Generic type aliases

**Advanced Generics** (TS 4.7+):

- Const type parameters (`<const T>`) - TS 5.0
- Variance modifiers (`in`, `out`) - TS 4.7
- Type instantiation expressions (`fn<T>`) - TS 4.7

### Function TypeScript Features

**Return Types**:

- Type predicates (`x is T`)
- Assertion signatures (`asserts x is T`)
- `asserts x`

**Overloads**:

- Function overloads
- Method overloads
- Constructor overloads

---

## Explicit Resource Management

A **finished (Stage 4)** TC39 proposal — `../../proposals/finished-proposals.md` lists it with an
expected publication year of 2027, so the grammar is settled but has not yet landed in a
published edition. It is absent from the `../../ecma262/` draft, and that proves nothing either
way: a finished proposal is one that "is (or soon will be) included in the latest draft", so
the draft's silence is not evidence of an earlier stage. Shipped in TS 5.2. Svelte's parser
rejects it; tsv is native — see
[conformance_svelte.md](./conformance_svelte.md#typescript-corrections).

- `using` declarations
- `await using` declarations
- `Symbol.dispose` (computed method syntax)
- `Symbol.asyncDispose` (computed method syntax)

---

# Future Work

Everything here is a TC39 **proposal** — no published ECMAScript syntax is missing (see
[Coverage](#coverage)). No stage is quoted, for the reason given there; `../../proposals/` is
the oracle. Each row below was verified against the binary rather than assumed.

## Not Parsed

Rejected outright — a parse error today:

| Proposal | Syntax |
| --- | --- |
| `throw` expressions | `const f = () => throw new Error()` |
| `do` expressions | `const x = do { 1; }`, `async do { … }` |
| Pattern matching | `match (x) { when 1: … }` |
| Pipeline operator | `a \|> f(%)` |
| `function.sent` metaproperty | `function* g() { const x = function.sent; }` |
| "Discard" (`void`) bindings | `const void = 1`, `const { a: void } = o` |
| Extractors | `const Foo(a) = x` |
| Module Expressions / Declarations | `const m = module { }` |
| Destructure Private Fields | `class C { #x; m(o) { const { #x: v } = o } }` |

## Parsed Generically, Not Modeled

RegExp modifiers (`(?i:pattern)`), duplicate named capture groups (`/(?<y>a)|(?<y>b)/`), and
the buffer-boundary proposal's `\A` / `\z` / `\Z` all **parse and round-trip today**: the
regex body is opaque, so no pattern grammar is ever consulted and no proposal can be "not
parsed" there — see [Regular Expressions](#regular-expressions) for what that costs on the
invalid-input side. The first two are in fact already standard (ES2025) and acorn accepts
them; they sit here rather than under Supported only because tsv models nothing about them.

## Not Coming

**Records and tuples** (`#{a: 1}` / `#[1, 2]`) was **withdrawn** — `../../proposals/inactive-proposals.md`
records it as "Withdrawn; subsumed by [Composites]", and Composites adds a library API, not
syntax. There is nothing here to parse, now or later. Named only so it is not re-added to the
list above.

---

# Out of Scope

## Strict Mode Only (with an explicit goal axis)

All code in Svelte scripts runs in strict mode (ES modules). tsv parses the
syntactic grammar; it enforces the *lexical* strict-mode restrictions but not the
strict-mode *early errors* — those still parse, with enforcement deferred to a
future diagnostics layer.

Strict and the *goal* (`Module` vs `Script`) are orthogonal — both goals are
strict. tsv defaults to `Module` (Svelte hard-wires it); a `Script` goal is
available (`parse_with_goal`, `--goal script`), where `await` is an ordinary
identifier and `import`/`export`/`import.meta` are errors. See
[conformance_test262.md](./conformance_test262.md#design-decision-strict-mode-only-explicit-goal-axis).

Rejected by the parser today:

- `with` statement — SyntaxError
- Legacy octal literals (`0777`) — SyntaxError (use `0o777`)

Early errors that still parse (not yet enforced):

- Octal escape sequences in strings (`'\07'`)
- Duplicate parameter names (`function f(a, a) {}`)
- Reserved words as identifiers (`var public = 1`)
- `delete` of a plain name (`delete x`)
- Invalid regular expressions — an unknown or repeated flag (`/a/qqq`, `/a/gg`), or a body the
  Pattern grammar rejects (`/(?zz:a)/`, `/a{2,1}/u`). This is the `IsValidRegularExpressionLiteral`
  early error; the lexical production is satisfied, so it is deferred like the rest, not a
  grammar hole. See [Regular Expressions](#regular-expressions)

Runtime-only (never a parse concern): `arguments.callee`, assigning to undeclared
variables.

The unenforced leaks only matter for standalone JS — Svelte/TS module context is
strict, so the real compiler would still flag them. When the diagnostics layer
lands, each early-error row gets an `input_invalid_*` fixture.

---

# Compatibility

Parse output matches acorn-typescript (the parser Svelte uses for `<script lang="ts">`) and formatting matches Prettier, except for the intentional divergences cataloged in [conformance_svelte.md](./conformance_svelte.md) and [conformance_prettier.md](./conformance_prettier.md).

## Svelte AST Integration

- TypeScript blocks in `<script lang="ts">`
- Type annotations in expressions
- Generic components (`<script lang="ts" generics="T">`)

## Intentional Differences

**`<const T>` in classes**: The tsv parser supports const type parameters on classes (`class Foo<const T>`), but acorn-typescript doesn't. See `typescript/generics/const_type_param_class_svelte_divergence/`.

**Parameter decorators**: Parsed as syntax (legacy-TypeScript, predating the TC39 decorators proposal) and attached to the parameter's `decorators` — see `tests/fixtures/typescript/typescript_specific/decorators/parameter/`. tsv accepts decorators in every member position (class, method, field, accessor, auto-accessor) and on parameters in the positions acorn parses them (function/method/constructor/object-method/ambient params), while **rejecting** parameter decorators where acorn + tsc + prettier all reject — arrow parameters and type-member signatures (see the boundary note under §Decorators).

---

# Appendix: Test262 Feature Mapping

Key test262 features relevant to parser/formatter:

- `arrow-function` — Expressions; ES2015
- `async-functions` — Functions; ES2017
- `async-iteration` — Iteration; ES2018
- `BigInt` — Literals; ES2020
- `class` — Declarations; ES2015
- `class-fields-private` — Classes; ES2022
- `class-fields-public` — Classes; ES2022
- `class-methods-private` — Classes; ES2022
- `class-static-block` — Classes; ES2022
- `class-static-fields-private` — Classes; ES2022
- `computed-property-names` — Objects; ES2015
- `const` — Declarations; ES2015
- `decorators` — Classes; TC39 proposal (test262 flags it under "Proposed language features")
- `default-parameters` — Functions; ES2015
- `destructuring-assignment` — Patterns; ES2015
- `destructuring-binding` — Patterns; ES2015
- `dynamic-import` — Modules; ES2020
- `exponentiation` — Operators; ES2016
- `for-of` — Statements; ES2015
- `generators` — Functions; ES2015
- `hashbang` — Comments; ES2023
- `import-attributes` — Modules; ES2025
- `import.meta` — Modules; ES2020
- `let` — Declarations; ES2015
- `logical-assignment-operators` — Operators; ES2021
- `new.target` — Expressions; ES2015
- `numeric-separator-literal` — Literals; ES2021
- `object-rest` — Patterns; ES2018
- `object-spread` — Objects; ES2018
- `optional-catch-binding` — Statements; ES2019
- `optional-chaining` — Expressions; ES2020
- `regexp-dotall` — RegExp; ES2018
- `regexp-lookbehind` — RegExp; ES2018
- `regexp-match-indices` — RegExp; ES2022
- `regexp-named-groups` — RegExp; ES2018
- `regexp-unicode-property-escapes` — RegExp; ES2018
- `regexp-v-flag` — RegExp; ES2024
- `rest-parameters` — Functions; ES2015
- `super` — Classes; ES2015
- `Symbol` — Primitives; ES2015
- `template` — Literals; ES2015
- `top-level-await` — Modules; ES2022
- `explicit-resource-management` — Statements; finished (Stage 4) proposal, publication expected 2027. test262 still files it under "Proposed language features" because the spec draft has not merged it yet — that placement tracks the draft, not the stage

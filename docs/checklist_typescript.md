# TypeScript Language Support

Comprehensive reference for TypeScript/JS language features supported by tsv's parser and formatter.

## Coverage

All strict-mode ECMAScript 2024 and TypeScript 5.x syntax features are supported, as enumerated below; sloppy-mode constructs are excluded by design (see [Out of Scope](#out-of-scope)). ECMAScript conformance is measured against test262 (see [conformance_test262.md](./conformance_test262.md)). Stage 2 proposals and experimental features are listed under [Future Work](#future-work).

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

- Basic patterns (`/pattern/`)
- Flags: `g`, `i`, `m`, `s`, `u`, `y`
- Flag: `d` (indices) - ES2022
- Flag: `v` (unicodeSets) - ES2024
- Character classes (`[a-z]`, `\d`, `\w`)
- Escapes in patterns
- Division vs regex disambiguation

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

### Update Expressions

- Prefix increment (`++x`)
- Prefix decrement (`--x`)
- Postfix increment (`x++`)
- Postfix decrement (`x--`)

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
- Postfix `++`/`--` restrictions
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

ES2015 module syntax with ES2024 additions.

### Import Declarations

- Default import (`import x from 'mod'`)
- Named imports (`import {a, b} from 'mod'`)
- Namespace import (`import * as ns from 'mod'`)
- Combined (`import x, {a} from 'mod'`)
- Renamed imports (`import {a as b} from 'mod'`)
- String import name (`import {'str' as b} from 'mod'`) - ES2022
- Side-effect import (`import 'mod'`)
- Import attributes (`import x from 'y' with {}`) - ES2024

### Export Declarations

- Named exports (`export {a, b}`)
- Renamed exports (`export {a as b}`)
- Declaration exports (`export const x = 1`)
- Default export (`export default expr`)
- Default function (`export default function() {}`)
- Default class (`export default class {}`)
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

**Decorators** (ES2023/TS 5.0):

- Class decorators (`@decorator class C {}`)
- Method decorators
- Property decorators
- Accessor decorators
- Decorator factories (`@decorator()`)
- Parameter decorators (`fn(@dec x: T)`)

Note: Parameter decorators are legacy-TypeScript syntax (not part of the ES2023 decorator standard), but tsv parses them — the parser attaches them to the parameter's `decorators`, covered by `tests/fixtures/typescript/typescript_specific/decorators/parameter/`.

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
- Interface merging

### TypeScript-Only Imports/Exports

- `import type { T } from 'mod'`
- `import { type T } from 'mod'`
- `export type { T }`
- `export type * from 'mod'`
- `import X = require('mod')`
- `import X = Namespace.Name`
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

Stage 3 proposal, widely supported. Spec: TC39 Explicit Resource Management.

- `using` declarations
- `await using` declarations
- `Symbol.dispose` (computed method syntax)
- `Symbol.asyncDispose` (computed method syntax)

---

# Future Work

Stage 2 proposals and experimental features tsv does not yet parse.

## Stage 3 Proposals (Not Widely Adopted)

- Import source phase (`import source x from "mod"`)
- Deferred import evaluation (`import defer * as ns from "mod"`)
- RegExp modifiers (`(?i:pattern)` inline modifiers)

## Stage 2 Proposals

- `throw` expressions
- `do` expressions
- Pattern matching
- Records and tuples
- Pipeline operator

---

# Out of Scope

## Strict Mode Only

All code in Svelte scripts runs in strict mode (ES modules). tsv parses the
syntactic grammar; it enforces the *lexical* strict-mode restrictions but not the
strict-mode *early errors* — those still parse, with enforcement deferred to a
future diagnostics layer.

Rejected by the parser today:

- `with` statement — SyntaxError
- Legacy octal literals (`0777`) — SyntaxError (use `0o777`)

Early errors that still parse (not yet enforced):

- Octal escape sequences in strings (`'\07'`)
- Duplicate parameter names (`function f(a, a) {}`)
- Reserved words as identifiers (`var public = 1`)
- `delete` of a plain name (`delete x`)

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

**Parameter decorators**: Parsed as syntax (legacy-TypeScript, predating the ES2023 decorator standard) and attached to the parameter's `decorators` — see `tests/fixtures/typescript/typescript_specific/decorators/parameter/`. tsv accepts all decorator positions: class, method, field, accessor, auto-accessor, and parameter.

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
- `decorators` — Classes; ES2023
- `default-parameters` — Functions; ES2015
- `destructuring-assignment` — Patterns; ES2015
- `destructuring-binding` — Patterns; ES2015
- `dynamic-import` — Modules; ES2020
- `exponentiation` — Operators; ES2016
- `for-of` — Statements; ES2015
- `generators` — Functions; ES2015
- `hashbang` — Comments; ES2023
- `import-attributes` — Modules; ES2024
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
- `explicit-resource-management` — Statements; Stage 3

# Directives

tsv honors in-source comments that suppress formatting for a piece of code. The
directives are recognized in every language tsv formats — TypeScript (`<script>`
and the JS/TS family: `.ts` / `.svelte.ts` / `.mts` / `.cts` / `.js` / `.mjs` /
`.cjs`), CSS (`<style>` and `.css`), and Svelte templates.

Like everything else in tsv, the directives are **not configurable**: they are
always active and cannot be turned off.

## `format-ignore`

Put a `format-ignore` comment immediately before a construct to emit it verbatim
instead of formatting it. The marked construct keeps its original spacing, line
breaks, and alignment; everything else in the file is formatted normally.

```svelte
<script lang="ts">
	// format-ignore
	const matrix = [
		1, 0, 0,
		0, 1, 0,
		0, 0, 1,
	];
</script>

<style>
	/* format-ignore */
	.grid   {   grid-template:   'a b' 1fr / auto;   }
</style>

<!-- format-ignore -->
<div    class="a"    data-attr="value" />
```

The comment delimiters follow the host language — `//` or `/* … */` in
TypeScript, `/* … */` in CSS, and `<!-- … -->` in Svelte templates.

### Placement

Where the comment sits decides what it freezes. The rule is total and has no
exceptions:

- **On its own line** (the only thing on its line, whitespace aside): the
  directive freezes the construct that follows it.
- **Anywhere else it is inert.** A directive sharing its line with anything
  else — trailing a statement, a list member, a separator, an opening `{`
  (`function f() { // format-ignore`), or a declaration head
  (`type A = // format-ignore`), or glued directly before a value
  (`let v: /* format-ignore */ {…}`) — is an ordinary comment: the surrounding
  code formats normally.

To freeze a construct, put the directive alone on the line above it.

### On type-member lists

The directives also target individual **members** of a type-member list — a
union or intersection, a tuple, a type-parameter declaration
(`function f<T, U>`), or a type-argument list (`Foo<A, B>`, `fn<A, B>(x)`): an
own-line directive in the list's leading gap or between members freezes the
**next member** only — the rest of the list keeps formatting normally,
separators included. The member freezes **whole**, whatever its shape — a tuple
element or type argument that is itself a union freezes as one item, operators
and all.

```ts
type T =
	// format-ignore
	| { x:1, y:2 }   // ← frozen verbatim
	| B              // ← formatted normally
	| C;

type U = [
	a,
	// format-ignore
	{ x:1, y:2 },    // ← frozen verbatim; the `,` stays parent-owned
	b
];
```

### On type heads

The same placement rule works between a head token and the single type it
introduces — a type annotation's `:`, a type alias's `=`, a type parameter's
`extends` constraint or `=` default, a named tuple member's `label:`, and a
mapped type's `]:` value. An own-line directive there freezes the type that
follows (a union or intersection child follows the member rules above
instead), and the directive stays where the author put it:

```ts
let v:
	// format-ignore
	{ x:1,  y:2 };   // ← frozen verbatim
```

Inside a mapped type, a directive above the `[K in ...]: V` clause freezes the
whole clause; a directive inside the bracket freezes just the `K in ...`
binding.

### On annotation heads

A directive can also sit on the other side of a `:` — in the gap *before* it,
between a head and its annotation. That reaches four heads: a binding (class
property, parameter, variable, index-signature key), a property signature, an
index signature's value `:`, and a signature's return type. The whole `: type`
freezes there, since that is what the directive precedes — a union or
intersection value included, and an optional `?` marker too:

```ts
interface I {
	a
		// format-ignore
		: { x:1,  y:2 };   // ← the whole `: { … }` frozen verbatim
}

function fn()
	// format-ignore
	: { x:1,  y:2 } {
	return { x: 1, y: 2 };
}
```

Such a gap only exists when a line comment already pushed the `:` onto its own
line, so this is a rare shape in practice.

### On parameter lists

A parameter list is a member list too, so the same rule applies: an own-line
directive after the `(` or between two parameters freezes the **next parameter**
only. The whole parameter freezes — its modifiers, decorators, `?`, default,
rest `...`, and type annotation are all part of what the directive precedes:

```ts
function fn(
	p: T,
	// format-ignore
	q: { x:1,  y:2 },   // ← frozen verbatim; the `,` stays parent-owned
	r: U
) {}
```

Every parameter list is covered — functions, methods, arrows, `{#snippet}`
parameters, method / call / construct signatures, function and constructor
types, and an index signature's `[key: T]`. A single parameter that would
normally hug (`fn({ a, b }: T)`) expands instead, so the directive keeps its own
line. A directive written *between* a parameter's decorators and its binding
freezes just the binding, leaving the decorators to format normally.

### On argument and element lists

Call, `new`, and dynamic-`import()` **arguments** and array literal / array
pattern **elements** are member lists too, under the same rule: an own-line
directive after the `(` / `[` or between two items freezes the **next item**
only.

```ts
fn(
	a,
	// format-ignore
	{ x:1,  y:2 },   // ← frozen verbatim; the `,` stays parent-owned
	b
);
```

A spread or rest `...` is part of what the directive precedes, so it rides inside
the frozen slice (`...  a  .  b` is kept verbatim). An argument that needs
clarity parens keeps them around the frozen slice (`(a = b  +  c)`). An
argument or element that would normally hug (`fn({ a, b })`, `new A([a, b])`)
expands instead, so the directive keeps its own line. An array hole contributes
only its comma, so the element after one still freezes.

### On module and declarator lists

Named **import / export specifiers**, a `with { … }` clause's **import
attributes**, and a variable declaration's **declarators** are member lists
too — same rule, an own-line directive freezes the **next item** only:

```ts
import {
	aaa as a1,
	// format-ignore
	bbb   as   b1,
	ccc as c1
} from './a';

const a = 1,
	// format-ignore
	b   =   2,
	c = 3;
```

The whole item freezes: an inline `type` modifier, a string specifier, an
attribute's key and value, a declarator's annotation, initializer, or
destructuring binding. A `for` header's init clause is the same declarator list.

The first item's gap opens just past the keyword, so a directive written between
`const`/`let`/`var` and the first declarator — or between `import` and a
`* as ns` namespace binding — freezes that item too. tsv keeps the directive on
its own line there, leaving the keyword alone on the line above; pulled up beside
the keyword it would be inert.

That last part holds in **every** declaration-header gap, including ones where
nothing freezes (`function`, `class`): a directive is never reflowed onto the
line above it, so its placement — the thing that decides whether it is honored —
is always the one you wrote.

### On value heads and sequence operands

A construct that holds a single value behind a delimiter of its own freezes that
**whole value** when an own-line directive sits in the gap — a `for` header's
init / test / update clauses and a for-in/for-of header's left clause, a
condition head's `(` (`if`, `else if`, `while`, `do…while`, a `switch`
discriminant, a `catch` parameter), a `return` / `throw` / `yield` operand
written in grouping parens, and a Svelte `{…}` value (`bind:`, `on:`, `class:`,
`style:`, an expression tag):

```ts
for (
	// format-ignore
	i  =  0;
	i < 10;
	i++
) {
	fn();
}

if (
	// format-ignore
	aaa  &&  bbb
) {
	fn();
}
```

```svelte
<div
	class:active={
		// format-ignore
		a  &&  b
	}
></div>
```

The delimiter that closes the value — the header's `;`, the `in`/`of` keyword,
the condition's `)`, the grouping `)`, the closing `}` — is parent-owned and
stays outside the frozen slice, and a sibling clause or attribute the freeze does
not reach still reformats. Parens the printer supplies for clarity are
parent-owned too: an assignment condition still prints as `if ((a = b))` around
the frozen slice. As in a declaration header, tsv keeps the directive on its own
line rather than pulling it up beside the `{`, where it would be inert.

A comma **sequence** is a member list inside that: an own-line directive between
two operands freezes the **next operand** only.

```ts
fn(
	(a,
	// format-ignore
	b  (  1  ),
	c)
);
```

At a sequence's leading gap the directive leads the *sequence* rather than its
first operand, so the whole sequence freezes — the value-head rule above. A
sequence prints its own grouping parens, and they are re-synthesized around a
frozen operand so its grouping survives.

### On assignment-family value heads

An assignment operator is a delimiter like any other, so an own-line directive in
an `=`→value or `:`→value gap freezes the **whole value** — a declarator
initializer, an assignment RHS (including a compound operator and each segment of
a chain), an object property value, a class field value, and a default value:

```ts
const aaa =
	// format-ignore
	bbb  +  ccc;

obj = {
	ddd:
		// format-ignore
		eee  +  fff
};

class Single {
	ggg =
		// format-ignore
		hhh  +  iii;
}

function fn(
	jjj =
		// format-ignore
		kkk  +  lll
) {}
```

```svelte
{@const mmm =
	// format-ignore
	nnn  +  ooo}
```

The binding, the operator and the enclosing list are parent-owned and stay
outside the frozen slice, so a sibling declarator or property the freeze does not
reach still reformats. Parens the printer supplies for clarity stay outside too:
an assignment initializer still prints as `const aaa = (bbb = ccc)` around the
frozen slice.

### On statement positions

Statements follow the same rule. In a statement **list** — a `switch` body's
cases, a case label's consequent statements, and (already) a program or block
body — an own-line directive freezes the **following** statement or case:

```ts
switch (aaa) {
	// format-ignore
	case   1:
		fn(  bbb  );
	case 2:
		fn(ccc);
}
```

At a statement **head** it freezes the single statement, clause or body that
follows — the consequent and alternate of an `if`, any loop's body, a labeled
statement's body, a `catch` or `finally` clause, and a class body:

```ts
if (aaa)
	// format-ignore
	fn(  bbb  );

while (aaa)
	// format-ignore
	fn(  bbb  );

lll:
// format-ignore
for (;;) {
	fn(  ccc  );
	break lll;
}

try {
	fn(aaa);
}
// format-ignore
catch (  eee  ) {
	fn(  ddd  );
}

class Aaa
// format-ignore
{
	mmm(  ) {}
}
```

A `case` label rides inside its own frozen case, and a class name and `extends`
clause stay parent-owned outside the frozen body — so the siblings the freeze
does not reach still reformat.

One position is inert: a directive between a **decorator** and its declaration
freezes nothing, because the decorator belongs to the declaration and the gap is
inside the statement rather than before it.

### On declaration heads

The `export` and `export default` keywords introduce a declaration the same way,
so a directive in the gap after either freezes it. The keyword stays outside the
frozen slice; decorators written *after* it belong to the declaration and ride
inside:

```ts
export
	// format-ignore
	const  aaa  =  1;

export default
	// format-ignore
	@dec
	class  Ddd  {}
```

A decorator written *before* `export` is the inert case again: the declaration
begins at the decorator, so the `export`→class gap is inside it and a directive
there freezes nothing.

An expression statement the printer wraps in parens freezes the same way, with
the parens left outside the slice:

```ts
(
	// format-ignore
	{ aaa:  1 }
);
```

### On prefixed Svelte braced heads

A `{` that carries a prefix before its value is a head like any other, so an
own-line directive in the prefix→value gap freezes that whole value. The prefix,
the `as` clause, an `{#each}` key's parens and the closing `}` all stay outside
the slice, and a sibling the freeze does not reach still reformats:

```svelte
{@html
	// format-ignore
	aaa  +  bbb
}

<div
	{...
		// format-ignore
		ccc  .  ddd
	}
	{@attach
		// format-ignore
		fn  (  eee  )
	}
></div>

{#if
	// format-ignore
	fff  &&  ggg
}
	text1
{/if}

{#each
	// format-ignore
	hhh  .  iii
as item (
	// format-ignore
	item  .  id
)}
	text2
{/each}
```

The same holds for `{@render }`, `{@debug }`, `{:else if }`, `{#key }` and
`{#await }`. A `{@debug}`'s slice is the identifier **list**, from the first
identifier to the last. `{@const}` belongs to the assignment family above (its
`=` is the delimiter), not here.

As in a declaration header, tsv keeps the directive on its own line rather than
pulling it up beside the prefix, where it would be inert — which is why a frozen
head breaks even when it would otherwise fit. Three gaps that look like heads
are not: the name in `{#snippet ⟨name⟩}` and the patterns in
`{#each … as ⟨pattern⟩}` / `{#await … then ⟨pattern⟩}` reject a comment outright,
in Svelte's parser as well as tsv's.

## `format-ignore-start` / `format-ignore-end`

In Svelte templates, a pair of range markers preserves every node between them:

```svelte
<!-- format-ignore-start -->
<div   >  hand   laid   out  </div>
<span  >  and   this   too  </span>
<!-- format-ignore-end -->
```

A range only takes effect at the top level of the template; markers nested inside
an element are treated as ordinary comments.

## `prettier-ignore` compatibility

For compatibility with prettier-authored code, tsv also honors the
`prettier-ignore` family — `prettier-ignore`, `prettier-ignore-start`, and
`prettier-ignore-end` — identically. `format-ignore` is the canonical tsv
spelling; `prettier-ignore` is kept so existing codebases keep working unchanged.
The two spellings are honored identically at every honored position — which
positions are honored is decided by [placement](#placement), never by spelling.

## See also

- [conformance_prettier.md §Format-ignore directive](./conformance_prettier.md#format-ignore-directive) — why the `format-ignore` spelling diverges from prettier

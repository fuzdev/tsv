<script lang="ts">
	// A paren shell heading the region, with a list behind it. tsc and acorn-typescript
	// both read the comparison chain `expected_svelte.json` records; tsv rejects it
	// whatever the shell holds and whatever follows the list, since where the printer
	// strips the shell the chain has no printed form that reads back as one.
	const a1 = f<(A)<C>>(x);
	const a2 = f<(A)<C>>`t`;
	const a3 = f<(typeof a)<C>>(x);
	const a4 = g(f<(A)<C>, D>(x));
	const a5 = f<(typeof (b))<C>>(x);
	const a6 = g(f<((b).c)<C>, D>(x));
	const a7 = f<(A | B)<C>>(x);
	const a8 = g(f<(A)<C, D>[0]>(x));
	const a10 = f<(A)<C> | D>(x);
	// a name whose list opens past a line break heads the region the same way
	const a9 = f<B
		<C>>(x);

	// A negative literal's list ahead of a call, a template or a bar: acorn-typescript
	// reads a generic call over the literal type, tsc a comparison chain.
	const b1 = f<-1<C>(e)>(x);
	const b2 = f<A | -1<C>(e)>(x);
	const b3 = f<-1<C> | D>(x);
	const b4 = f<A[() => -1<C>]>(x);
	// and a member tail glued to its digits, read the same two ways
	const b5 = f<-1..x>(x);
	const b6 = f<A[-1..x]>(x);
	// and its spaced postfix in an index whose body opens as a type
	const b7 = f<A[B | -1(e)]>(x);
	const b8 = f<A[-1 .x]>(x);
	const b9 = f<A[A[-1`t`]]>(x);

	// An import head that is no import type: tsc reads a generic call, acorn-typescript a
	// comparison chain over a dynamic import or the meta-property.
	const c1 = f<import(c)>(x);
	const c2 = a < b | import(c) > (x);
	const c3 = f<import.meta>(x);

	// A second list behind a nested one: acorn-typescript reads a comparison chain over
	// an instantiation, tsc rejects the first line and reads another chain in the second.
	const d1 = f<A<B><C>>(x);
	const d2 = f<A<B><C> + 1>(x);
	// behind a keyword type's own list heading the region, and inside an index where an
	// operand follows the second list
	const d3 = g(f<string<C><D>, B>(x));
	const d4 = f<A[b<C><D> + 1]>(x);
	const d5 = f<A[(b)<C><D> + 1]>(x);

	// An import type's list past a line break: acorn-typescript takes it and reads a
	// generic call, tsc ends the type at the break and reads a comparison chain.
	const e1 = f<A | import('m').B
		<C>>(x);

	// Two more tokens the parsers part on, in an index whose body opens as a type: an arrow
	// function's parameter default (a function type to tsc, an arrow function to
	// acorn-typescript) and a meta-property (a type reference to acorn-typescript, an error
	// to tsc).
	const f1 = f<A[A[(a = 1) => 0]]>(x);
	const f2 = f<A[B | new.target]>(x);
</script>

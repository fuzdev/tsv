<script lang="ts">
	// A block-body first callback expands "first" and keeps a cast-wrapped EMPTY seed
	// inline after `}` only when the cast is "hopefully short"
	// (isHopefullyShortCallArgument): the type — after unwrapping an array element type
	// and descending into a lone type argument — must be simple, and the wrapped
	// expression must be simple at depth 1. An angle-bracket assertion is never short.

	// bare type reference is simple — seed stays inline
	foo(()=>{doThing();},{} as T);

	// one type argument, descended into — `B` is simple, seed stays inline
	foo(()=>{doThing();},{} satisfies A<B>);

	// array element type unwrapped — `T` is simple, seed stays inline
	foo(()=>{doThing();},{} as T[]);

	// two type arguments — not simple, so all args break
	foo(()=>{doThing();},{} as A<B,C>);

	// wrapped expression is a call with an argument — not simple at depth 1, all args break
	foo(()=>{doThing();},bar(a) as T);

	// an angle-bracket assertion is never short — all args break
	foo(()=>{doThing();},<T>{});

	// plain empty object tail — simple, stays inline
	foo(()=>{doThing();},{});

	// the same rule through the member-chain printer
	arr.filter((x)=>!!x).reduce((acc,x)=>{doThing();return acc;},<T>{});
</script>

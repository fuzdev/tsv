<script lang="ts">
	// A negative literal type keeps its parens as an array element type
	type A = ((-1))[];
	type B = (((-1n)))[];
	type C = ((-0.5)[])[];
	type D = readonly ((-1)[]);
	type E = [...((-1)[])];

	// It keeps them as an indexed-access object type too
	type F = ((-1))[K];
	type G = ((-1n))['a'];
	type H = ((-1)[K])[];
	type J = ((-1))[0];

	// The same holds in every type position
	let a: ((-1)[]) = [];
	let b = fn1<((-1)[])>(c);
	let d = c as ((-1)[K]);
	let e = fn1<((-1))[K]>(c);
	let f = fn1<((-1))[0] | (A)>(c);
	function fn2(): ((-1))[] {}
	type I<T = ((-1)[])> = J<((-1))[K]> | [((-1)[])];
</script>

{c as ((-1))[]}
{c satisfies ((-1)[K])}

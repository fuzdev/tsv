<script lang="ts">
	// A function/constructor type hugs its sole parameter for every shape prettier's
	// `shouldHugTheOnlyFunctionParameter` accepts — a mapped-type annotation and a
	// destructuring pattern, not only an object-type literal.

	// Mapped type, authored expanded
	type A=(a:{
[K in T]:U;
})=>void;
	type B=new (a:{
[K in T]:U;
})=>void;

	// Optional parameter, modifiers
	type C=(a?:{
readonly [K in T]?:U;
})=>void;

	// Redundant paren shell
	type D=(a:({
[K in T]:U;
}))=>void;

	// Flat mapped type: 100 chars stays inline, 101 breaks inside the mapped type
	type E=(a:{[K in Taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]:U})=>void;
	type F=(a:{[K in Taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]:U})=>void;

	// Array pattern with a tuple annotation: the tuple breaks, the pattern hugs
	type G=([aaaaaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbbbbbbbb]:[aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb])=>void;
</script>

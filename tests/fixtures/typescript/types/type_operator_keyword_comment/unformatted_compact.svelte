<script lang="ts">
	// Type predicate
	function fn1(x: any): x is/* c */A {}

	// Asserts predicate with is
	function fn2(x: any): asserts x is/* c */A {}

	// Asserts predicate without is
	function fn3(x: any): asserts/* c */x {}

	// keyof operator
	type A = keyof/* c */B;

	// typeof operator
	type B = typeof/* c */x;

	// infer keyword
	type C = string extends infer/* c */T ? T : never;

	// unique symbol
	declare const sym: unique/* c */symbol;
</script>

<script lang="ts">
	// Angle-bracket inside angle-bracket: needs parens
	let a = <T>(<U>foo());

	// Angle-bracket inside as/satisfies: needs parens
	let b = (<T>foo()) as U;
	let c = (<T>foo()) satisfies U;

	// As/satisfies inside angle-bracket: needs parens
	let d = <T>(foo() as U);
	let e = <T>(foo() satisfies U);

	// As/satisfies chaining: no parens needed (left-associative)
	let f = foo() as A as B;
	let g = foo() satisfies A satisfies B;
	let h = foo() as A satisfies B;
</script>

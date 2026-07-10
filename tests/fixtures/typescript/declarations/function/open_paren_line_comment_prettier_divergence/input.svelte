<script lang="ts">
	// function declaration — comment stays trailing `(`
	function fn( // c
		a: number
	) {}

	// function expression
	const expr = function ( // c
		a: number
	) {};

	// arrow function
	const arrow = ( // c
		a: number
	) => a;

	// a `( // c` comment disables the single-pattern hug (a comment forces the
	// list to expand) and is preserved on the `(` line
	function hug( // c
		{ a, b }: T
	) {}
</script>

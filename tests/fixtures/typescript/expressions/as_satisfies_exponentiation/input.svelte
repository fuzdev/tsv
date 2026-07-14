<script lang="ts">
	// A cast binds tighter than `**`, so as the left operand it needs parens
	let a = (x as number) ** 2;
	let b = (x satisfies number) ** 2;

	// As the right operand the parens are load-bearing: without them `as` binds
	// looser than `**` and swallows the whole exponentiation
	let c = 2 ** (x as number);
	let d = (2 ** x) as number;
</script>

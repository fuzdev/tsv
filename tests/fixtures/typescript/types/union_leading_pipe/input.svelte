<script lang="ts">
	// Leading pipe - removed by formatter
	type A = string | number;

	// Leading pipe in parenthesized union - parens and pipe removed
	type B = string | number;

	// Leading pipe in intersection context - parens preserved for precedence
	type C = A & ({ a: string } | { b: number });

	// Leading ampersand in intersection
	type D = { a: string } & { b: number };

	// Leading pipe in destructuring type annotation - parens preserved for precedence
	const { a }: A & ({ a: string } | { a: number }) = x;
</script>

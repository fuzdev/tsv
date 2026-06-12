<script lang="ts">
	// Unary minus with exponentiation requires parens (ambiguous otherwise)
	let a = (-2) ** 3;
	let b = (+2) ** 3;
	let c = (~2) ** 3;

	// Right-associative chaining - prettier adds explicit parens
	let d = 2 ** (3 ** 4);

	// Explicit grouping preserved
	let e = (2 ** 3) ** 4;
</script>

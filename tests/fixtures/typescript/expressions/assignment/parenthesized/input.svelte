<script lang="ts">
	let result;
	let a;
	let b: { c: number };

	// Assignment as initializer value (parentheses required for clarity)
	const j1 = (result = a < b.c);
	const j2 = (result = a);
	const j3 = (result = a + b.c);

	// Compound assignment as initializer
	const k1 = (a += 1);
	const k2 = (a -= b.c);
	const k3 = (a *= 2);

	// Nested assignments
	const n = (a = result = 1);

	// Assignment with logical operators
	const l1 = (a ||= b.c);
	const l2 = (a &&= b.c);
	const l3 = (a ??= b.c);
</script>

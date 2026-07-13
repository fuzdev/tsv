<script lang="ts">
	// Simple type interpolation
	type A = `${SomeType}`;

	// Multiple interpolations
	type B = `prefix_${TypeA}_middle_${TypeB}_suffix`;

	// Conditional type inside interpolation
	type C = `${A extends B ? C : D}`;

	// Union type inside interpolation
	type D = `${A | B | C}`;

	// Nested template literal type
	type E = `outer_${`inner_${Deep}`}`;

	// Adjacent interpolations — the empty quasi between them (cooked/raw '')
	type F = `${A}${B}`;
</script>

<script lang="ts">
	// Line comment trailing on true branch stays trailing
	type A = B extends T
		? foo // c
		: bar;

	// Block comment between true branch and : stays inline
	type B = B extends T ? foo /* c */ : bar;

	// Nested: trailing comment in inner conditional
	type C = A extends B
		? C extends D
			? foo // c
			: bar
		: baz;
</script>

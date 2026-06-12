<script lang="ts">
	// Line comment after : stays after : (preserves user intent)
	type A = B extends T
		? foo // c
		: bar;

	// Multiple line comments after : stay after :
	type B = B extends T
		? foo
		: // c1
			// c2
			bar;

	// Nested: comment after : in inner conditional
	type C = A extends B
		? C extends D
			? foo // c
			: bar
		: baz;
</script>

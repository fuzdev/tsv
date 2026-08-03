<script lang="ts">
	// A line comment between a mapped type's key name and its `in` keyword stays
	// after the name; the `in T` tail continues one indent level in.
	type A = {
		[K // c
		in T]: V;
	};

	// The `as` clause's before-keyword gap takes the same continuation.
	type B = {
		[K in T // c
		as U]: V;
	};

	// A stacked run stays with the name; the tail continues below the run.
	type C = {
		[K // c1
		// c2
		in T]: V;
	};
</script>

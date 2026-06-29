<script lang="ts">
	// Line comment after a mapped type's `in`/`as` keyword, the following type on
	// the next line.
	type M = {
		[K in // c
		T]: V;
	};
	type N = {
		[K in T as // c
		P]: V;
	};
</script>

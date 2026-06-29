<script lang="ts">
	// Line comment after a mapped type's `in`/`as` keyword, the following type on
	// the next line.
	type M = {
		[
			K in T // c
		]: V;
	};
	type N = {
		[
			K in T as P // c
		]: V;
	};
</script>

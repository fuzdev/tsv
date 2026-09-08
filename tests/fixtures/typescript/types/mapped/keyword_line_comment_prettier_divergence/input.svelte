<script lang="ts">
	// Line comment after a mapped type's `in`/`as` keyword: the brackets break, the
	// comment stays after the keyword and the following type takes the next line.
	type M = {
		[
			K in // c
			T
		]: V;
	};
	type N = {
		[
			K in T as // c
			P
		]: V;
	};
</script>

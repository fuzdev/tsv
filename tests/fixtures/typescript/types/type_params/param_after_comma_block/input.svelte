<script lang="ts">
	// Type param declaration: a block hugging the next parameter leads it
	function fn1<
		T, // t
		U,
		/* c */ V
	>() {}

	// Before-comma and after-comma blocks in one gap each keep their side
	function fn2<
		T, // t
		U /* c1 */,
		/* c2 */ V
	>() {}

	// Type argument list: same rule
	fn1<
		string, // t
		number,
		/* c */ boolean
	>();
</script>

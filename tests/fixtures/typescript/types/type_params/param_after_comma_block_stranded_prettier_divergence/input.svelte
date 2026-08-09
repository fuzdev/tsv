<script lang="ts">
	// A stranded after-comma block (the author left a newline before the next
	// parameter) stays trailing the comma; prettier relocates it before the comma
	function fn1<
		T, // t
		U, /* c */
		V
	>() {}

	// A before-comma block and a stranded after-comma block in the same gap each
	// stay on their own side of the comma; prettier relocates both before it
	function fn2<
		T, // t
		U /* c1 */, /* c2 */
		V
	>() {}

	// Type argument list: same rule
	fn1<
		string, // t
		number, /* c */
		boolean
	>();
</script>

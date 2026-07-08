<script lang="ts">
	// A stranded after-comma block (the author left a newline before the next
	// param) stays trailing the comma; prettier relocates it before the comma
	const fn1 = (
		argumentnumberoneeeeee, /* comment */
		argumentnumbertwooooooooooooooooooooooooooooo
	) => {};

	// A before-comma block and a stranded after-comma block in the same gap each
	// stay on their own side of the comma; prettier relocates both before it
	const fn2 = (
		argumentnumberoneeeeee /* c1 */, /* c2 */
		argumentnumbertwoooooooooooooooooooooooooo
	) => {};
</script>

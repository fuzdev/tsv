<script lang="ts">
	// A stranded after-comma block (the author left a newline before the next
	// element) stays trailing the comma; prettier relocates it before the comma
	type A = [
		string, // t
		number, /* c */
		boolean
	];

	// A before-comma block and a stranded after-comma block in the same gap each
	// stay on their own side of the comma; prettier relocates both before it
	type B = [
		string, // t
		number /* c1 */, /* c2 */
		boolean
	];
</script>

<script lang="ts">
	// A multiline block the author broke after keeps the break: `= value` drops to
	// a continuation line indented one level. Object shorthand default.
	const {
		a /* x
y */
			= 1
	} = obj;

	// Object non-shorthand default (the binding sits after the `:`).
	const {
		k: v /* x
y */
			= 1
	} = obj;

	// Array destructuring default.
	const [
		b /* x
y */
			= 1
	] = arr;

	// Standalone parameter default.
	function g(
		d /* x
y */
			= 1
	) {}

	// A run the multiline block leads: the later single-line block takes the same
	// continuation line, glued to the `=`.
	function h(
		e /* x
y */
			/* c */ = 1
	) {}

	// One whose `=` shares the comment's closing line stays glued.
	function i(
		f /* x
y */ = 2
	) {}
</script>

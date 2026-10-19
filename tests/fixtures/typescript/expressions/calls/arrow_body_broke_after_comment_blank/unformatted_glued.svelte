<script lang="ts">
	// A block comment glued to `=>` that the author broke after, then a blank line, before a
	// body with a hard break of its own: the break is forced, so the comment takes its own
	// line under `=>` and the blank survives with it — in every call that hugs the arrow.
	fn('x', (a) => /* c */

	fn1(`a
b`));

	new Fn('x', (a) => /* c */

	fn1(`a
b`));

	obj.fn1().fn2('x', (a) => /* c */

	fn1(`a
b`));

	fn((a) => /* c */

	fn1(`a
b`));
</script>

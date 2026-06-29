<script lang="ts">
	// Consecutive block comments in the gap after `(` collapse onto one line, and a
	// block stays glued to the argument it hugs — when nothing forces this multi-arg
	// call open (no line comment, no standalone own-line block).
	fn(/* t */ /* u */ a, b);

	// A blank line in the gap breaks the run and is preserved; the hugging block
	// still keeps its argument on the same line.
	fn(
		/* t */

		/* u */ c,
		d
	);
</script>

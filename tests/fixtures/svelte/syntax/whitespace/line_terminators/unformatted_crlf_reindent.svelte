<script module lang="coffee">
x = 1
y = 2
</script>

<script lang="ts">
	/* a
	 b */
	const a = `text1
text2`;

	// prettier-ignore
	const b = [ 1,
		2 ];

			/**
			 * c
			 */
	const c = 1;
</script>

<div>
	<!-- A line terminator normalizes to LF at every position, including the ones tsv emits
		by copying the author's source bytes rather than by rebuilding the line. -->
	<style lang="stylus">
a
  color red
	</style>
</div>

<style>
	/* a
	 b */
	div {
		color: red;
	}
</style>

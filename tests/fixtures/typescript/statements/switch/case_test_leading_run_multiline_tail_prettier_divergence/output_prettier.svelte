<script lang="ts">
	switch (a) {
		// a run the author broke after, ending in a multi-line comment glued to the test: the
		// break after the first comment is forced, and the test keeps its own layout
		case /* x */
		/* y
		 */ aaa ? bbb : ccc:
		// a test that breaks on its own forces the break too
		case /* x */
		{
			aaa: 1
		}:
	}
</script>

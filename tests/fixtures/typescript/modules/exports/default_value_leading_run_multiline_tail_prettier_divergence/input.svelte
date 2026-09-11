<script lang="ts">
	// a run the author broke after, ending in a multi-line comment glued to the value: the break
	// after the first comment is forced, and the value keeps its own layout below the comment
	export default /* x */
		/* y
		 */ aaa ? bbb : ccc;
</script>

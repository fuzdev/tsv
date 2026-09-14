<script lang="ts">
	// a run the author broke after, ending in a multi-line comment glued to the value: the break
	// after the first comment is forced, and the value keeps its own layout below the comment
	export = /* x */
	/* y
	 */ aaa ? bbb : ccc;

	// the run the author GLUED the other way round — a multi-line block ahead of a single-line
	// one, broken after that — keeps the glue, and the value hangs one level under the keyword
	export = /* x
y */ /* z */
	ddd ? eee : fff;
</script>

<script lang="ts">
	// A `>>>` is disqualified as a `>`-led follower too, though here no re-scan reaches it:
	// acorn refuses the whole `<</>>/>>>` token class after a would-be close, and even the
	// unspaced `f<T>>>> 1` is a form neither parser takes.
	const a = f<T> >>> 1;
</script>

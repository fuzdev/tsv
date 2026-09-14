<script lang="ts">
	// A `>>` is disqualified for the same reason a lone `>` is, one re-scan over: the close
	// and the `>>` would read as `>>>`, which is what the unspaced `f<T>>> 1` is — both
	// parsers take that one, as `f < (T >>> 1)`. Spaced, both reject.
	const a = f<T> >> 1;
</script>

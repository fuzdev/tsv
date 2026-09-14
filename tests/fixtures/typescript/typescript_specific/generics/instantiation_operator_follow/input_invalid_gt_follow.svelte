<script lang="ts">
	// The grammar disqualifies a `>`-led follower after a type argument list, whatever the
	// spacing: the close and the `>` would be ambiguous with a re-scanned `>>` operator. The
	// re-scan is what the UNSPACED `f<T>> 1` actually is — `f < (T >> 1)`, which both parsers
	// take; spaced, the list has no reading left and both reject.
	const a = f<T> > 1;
</script>

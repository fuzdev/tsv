<script lang="ts">
	// The glued twin of the spaced `f<T> >= c` that acorn-typescript reads as
	// `(f<T>) >= c`: glued, the longest punctuator is `>>=`, so the source reads
	// `f < T >>= c`, and both parsers reject. Only the spelling with the gap is an
	// instantiation follow.
	f<T>>= c;
</script>

<script lang="ts">
	// Glued, the close is no `>` at all: the longest punctuator at that position is
	// `>>>=`, so the source reads `f < T >>>= c` — a comparison assigned to, which no
	// production derives. Splitting the close off the operator reads an instantiation
	// no parser has.
	f<T>>>= c;
</script>

<script lang="ts">
	// A close glued to `==` is a `>=` to tsc: its re-scan joins the `>` to the `=` after it,
	// so no list closes and the rest reads `f < T >= = c`. Spaced, `f<T> == c` is an
	// instantiation compared, which both parsers take.
	f<T>==c;
</script>

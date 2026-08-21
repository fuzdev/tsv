<script lang="ts">
	// A type keyword opens type arguments only when the `<`'s first token IS that
	// keyword. `string$` and `stringµ` merely begin with one — `$` and every
	// non-ASCII identifier character continue a name — so each `<` below is the
	// less-than operator and the line is a comparison, not an instantiation.
	const a1 = p < string$ ? q : r > s;
	const a2 = p < stringµ ? q : r > s;
	const a3 = p < string$ - 1 > s;
	const a4 = p < nullµ ? q : r > s;
	const a5 = p < true$ ? q : r > s;

	// The bare keyword still opens them.
	const b1 = fn<string>();
	const b2 = fn<string | number>();
	const b3 = fn<readonly string[]>();
</script>

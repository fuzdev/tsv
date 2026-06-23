<script>
	// trailing comments before the separator comma are all kept before it, even
	// when an earlier comment contains a comma
	const { a /* , */ /* x */, b } = obj;

	// array pattern: same
	const [c /* , */ /* y */, d] = arr;
</script>

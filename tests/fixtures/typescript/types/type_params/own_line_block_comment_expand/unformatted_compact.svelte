<script lang="ts">
	// Own-line block comment between type params forces expansion
	function fn1<
		T,
		/* c */
		U,
	>() {}

	// Own-line block comment before first param forces expansion
	function fn2<
		/* c */
		T,
		U,
	>() {}

	// Multiple own-line block comments between params
	function fn3<
		T,
		/* c1 */
		/* c2 */
		U,
	>() {}

	// Same-line block comment does NOT force expansion (short)
	function fn4<T,/* c */ U>() {}

	// Inline block comment after comma does NOT force expansion
	function fn5<T,/* c */ U,V>() {}
</script>

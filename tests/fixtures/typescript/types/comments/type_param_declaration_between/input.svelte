<script lang="ts">
	// Leading comment before each param
	function fn1</* c1 */ T, /* c2 */ U, /* c3 */ V>() {}

	// Trailing comment after param, leading comment before next
	function fn2<T /* c1 */, /* c2 */ U>() {}

	// Trailing comment only (before comma)
	function fn3<T /* c */, U>() {}
</script>

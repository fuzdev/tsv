<script lang="ts">
	// An own-line block comment with the comma glued after leads the next type parameter
	function fn1<T
/* c */, U>(a: T, b: U) {}

	// Written before the comma instead, it trails the previous type parameter
	function fn2<T /* c */, U>(a: T, b: U) {}

	// A line comment expands the list - the comment still leads its parameter, on its line
	function fn3<T
/* c */, U // t
>(a: T, b: U) {}
</script>

<script lang="ts">
	// Block comment after type param name
	function fn1<T /* c */>() {}

	// Block comment before type param name
	function fn2</* c */ T>() {}

	// Block comment after extends
	function fn3<T extends /* c */ A>() {}

	// Block comment after default =
	function fn4<T = /* c */ A>() {}

	// Block comment after extends and after default
	type A<T extends /* c1 */ B = /* c2 */ C> = T;

	// Line comment after type param name (forces multiline)
	function fn5<
		T // c
	>() {}

	// Line comment after extends
	function fn6<
		T extends // c
			A
	>() {}

	// Line comment after default =
	function fn7<
		T = // c
			A
	>() {}

	// Multiple type params with comments
	function fn8<T extends /* c1 */ A, U = /* c2 */ B>() {}
</script>

<script lang="ts">
	// Short union - stays on one line
	type A = Foo | Bar;

	// Short union with leading pipe - normalized to single line
	type B = Foo | Bar | Baz;

	// Long union - breaks with leading |
	type VeryLongUnionType =
		| FooooooooooooooooLong
		| BarrrrrrrrrrrrrrrrrrrrLong
		| BazzzzzzzzzzzzzzzzzzzzLong
		| QuxxxxxxxxxxxxxxxxxxLong;

	// Intersection stays on line when fits
	type C = Foo & Bar & Baz;

	// Long intersection - breaks with trailing &
	type VeryLongIntersectionType = FooooooooooooooooLong &
		BarrrrrrrrrrrrrrrrrrrrLong &
		BazzzzzzzzzzzzzzzzzzzzLong &
		QuxxxxxxxxxxxxxxxxxxLong;
</script>

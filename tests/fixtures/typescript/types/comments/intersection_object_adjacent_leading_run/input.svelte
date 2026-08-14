<script lang="ts">
	// a comment glued to the member at an object-adjacent boundary rides inline after the `&`
	type A = a &
		// k
		{ x: 1 } & /* c1 */ { y: 2 };

	// the whole run rides, not just its head
	type B = a &
		// k
		{ x: 1 } & /* c1 */ /* c2 */ { y: 2 };

	// control: the same run authored glued to the `&`
	type C = a &
		// k
		{ x: 1 } & /* c1 */ { y: 2 };
</script>

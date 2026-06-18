<script lang="ts">
	// A non-last `new` argument with a block comment before the comma and a line
	// comment after it: the block trails the argument, the line comment trails the
	// comma. The block is neither dropped nor moved past the comma.
	new A(a/* c1 */,// c2
		b);

	// Two block comments before the comma, then a line comment after it.
	new A(a/* c1 *//* c2 */,// c3
		b);

	// A blank line after the commented arg routes through the blank-line args path;
	// the block must still be preserved before the comma there too.
	new A(a/* c1 */,// c2

		b);
</script>

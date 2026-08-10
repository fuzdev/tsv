<script lang="ts">
	// A glued pair of block comments takes a soft line before the param, so a list that fits
	// keeps the pair on the param's line.
	function fn1(
		/* c1 */ /* c2 */
		a
	) {}

	// The same between params.
	function fn2(
		a,
		/* c1 */ /* c2 */
		b
	) {}

	// A block the author isolated on its own line takes a hardline instead, so it forces
	// the list open.
	function fn3(
		/* c */
		a
	) {}

	// At exactly 100 columns the list still fits, so the pair keeps the param's line.
	function fn4(
		/* c1 */ /* c2 */
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong: string
	) {}

	// One column more and the list breaks - the soft line breaks with it.
	function fn5(
		/* c1 */ /* c2 */
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong: string
	) {}
</script>

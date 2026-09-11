<script lang="ts">
	// An author blank after a leading comment run survives, and the argument list opens.
	fn1(/* c1 */

	b);

	// Same for a glued run.
	fn2(/* c1 */ /* c2 */

	b);

	// `new` shares the rule, for a lone argument and for several.
	new C3(/* c1 */

	`a
b`);
	new C4(/* c1 */

	/* c2 */ (
	{
		a: 1
	}), c);

	// So does a member-chain call.
	obj.m5(/* c1 */ /* c2 */

	function () {
		b();
	});
	obj.m6(/* c1 */

	b, c);

	// No blank: the comment still takes its own line above a breaking argument.
	fn7(/* c1 */
	{
		a: 1
	}, c);
	new C8(/* c1 */
	{
		a: 1
	});

	// A blank inside the run, then a glued pair the author broke after: both breaks hold.
	fn9(/* c1 */

	/* c2 */ /* c3 */
	b, c);

	// A multi-line comment glued to the argument keeps the blank above it.
	fn10(/* c1 */

	/* c2
	*/ b, c);

	// No blank: the multi-line comment still takes its own line.
	fn11(/* c1 */
	/* c2
	*/ b, c);

	// A blank inside the argument's parens is not an author blank.
	fn12(/* c1 */ b, c);
	fn13(/* c1 */

	/* c2 */ b, c);

	// A lone argument below the same run: both breaks hold.
	fn14(/* c1 */

	/* c2 */ /* c3 */
	b);
</script>

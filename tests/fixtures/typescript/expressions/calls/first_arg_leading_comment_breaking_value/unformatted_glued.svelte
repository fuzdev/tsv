<script lang="ts">
	// A block run glued to `(` that the author broke after keeps its break when the
	// argument list opens: the run takes its own line ahead of the first argument.
	fn1(/* c1 */
	{
		b: 1,
		c: 2
	}, d1);

	// `new` shares the rule.
	new Fn2(/* c2 */
	{
		b: 1,
		c: 2
	}, d2);

	// Width alone breaks the list the same way: the run holds its own line and the
	// arguments follow (the whole call is over 100 wide as authored).
	fn3(/* c3 */
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccc);

	// A lone argument too wide to share the call's line breaks the same way.
	fn4(/* c4 */
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);

	// A glued run stays glued: `new` expands around a breaking argument exactly as
	// the plain call does, the run riding the argument's line.
	new Fn5(/* c5 */ function () {
		d3();
	});
</script>

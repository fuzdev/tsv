<script lang="ts">
	// A glued run the author gave its own line keeps that line when the lone argument breaks.
	fn1(/* c1 */ /* c2 */
	({
		a: 1
	}));

	// Same for an argument that breaks by width.
	fn2(/* c1 */ /* c2 */
	(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccccc));

	// Same for a template literal argument.
	fn3(/* c1 */ /* c2 */
	`a
b`);

	// The first comment broke after and the second is glued to the argument: the break holds.
	fn4(/* c1 */
	/* c2 */ ({
		a: 1
	}));

	// `new` shares the rule.
	new C5(/* c1 */
	/* c2 */ ({
		a: 1
	}));

	// So does a member-chain call.
	obj.m6(/* c1 */
	/* c2 */ ({
		a: 1
	}));
</script>

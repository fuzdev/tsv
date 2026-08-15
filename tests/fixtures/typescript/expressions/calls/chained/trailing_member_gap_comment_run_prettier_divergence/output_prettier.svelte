<script lang="ts">
	// a same-line `//` with another comment behind it IN THE SAME GAP: the collapse
	// would carry the block ahead of the deferred `//` and off its authored line
	/* c2 */
	fn() // c1
	.bar;

	// a same-line block ahead of it too — source order is c3, c4, c5
	/* c5 */
	fn() /* c3 */ // c4
	.bar;

	// in an initializer
	const a =
		/* c7 */
		fn() // c6
		.bar;

	// through an optional member and past a non-null on the call
	/* c9 */
	fn() // c8
	?.bar;
	/* c11 */
	fn()! // c10
	.bar;

	// CONTROL: a lone same-line `//` still takes the sanctioned collapse
	fn().bar; // c12

	// CONTROL: blocks only — nothing defers, so nothing can be reordered
	fn() /* c13 */ /* c14 */.bar;

	// the follower GLUED to the property is owned by it — printed by the member's own
	// doc rather than by this gap, which does not spare it from landing ahead of the
	// deferred `//`, so the gate asks the on-page axis rather than the to-emit one
	/* c16 */ fn() // c15
	.bar;
</script>

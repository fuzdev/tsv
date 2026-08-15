<script lang="ts">
	// a same-line `//` with another comment behind it IN THE SAME GAP: the collapse
	// would carry the block ahead of the deferred `//` and off its authored line
	fn() // c1
		/* c2 */
		.bar;

	// a same-line block ahead of it too — source order is c3, c4, c5
	fn() /* c3 */ // c4
		/* c5 */
		.bar;

	// in an initializer
	const a = fn() // c6
		/* c7 */
		.bar;

	// through an optional member and past a non-null on the call
	fn() // c8
		/* c9 */
		?.bar;
	fn()! // c10
		/* c11 */
		.bar;

	// CONTROL: a lone same-line `//` still takes the sanctioned collapse
	fn().bar; // c12

	// CONTROL: blocks only — nothing defers, so nothing can be reordered
	fn() /* c13 */ /* c14 */.bar;

	// the follower GLUED to the property is owned by it — printed by the member's own
	// doc rather than by this gap, which does not spare it from landing ahead of the
	// deferred `//`, so the gate asks the on-page axis rather than the to-emit one
	fn() // c15
		/* c16 */.bar;
</script>

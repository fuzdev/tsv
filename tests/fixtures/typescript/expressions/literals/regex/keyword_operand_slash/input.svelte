<script lang="ts">
	fn(() => {
		// After `typeof` the `/` opens a regex — the keyword is an operator, not an operand
		if (typeof /\// === 'object') return 'x';

		/* c1 */
		return null;
	});

	fn(() => {
		// Same after `void`
		if (void /\//) return 'x';

		/* c2 */
		return null;
	});

	fn(() => {
		// Same after the `in` operator
		if ('a' in /\//) return 'x';

		/* c3 */
		return null;
	});

	fn(() => {
		// An identifier before `/` is an operand, so this is division, not a regex
		if (aa / bb > 1) return 'x';

		/* c4 */
		return null;
	});

	fn(() => {
		// A reserved word used as a property name is an operand too
		if (a.in / bb > 1) return 'x';

		/* c5 */
		return null;
	});
</script>

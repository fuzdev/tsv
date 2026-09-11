<script lang="ts">
	// A multi-line comment glued to the argument keeps the break after the comment above it.
	fn1(
		/* c1 */
		/* c2
		 */ b
	);
	new C2(
		/* c1 */
		/* c2
		 */ b
	);
	obj.m3(
		/* c1 */
		/* c2
		 */ b
	);

	// The same with several arguments.
	fn4(
		/* c1 */
		/* c2
		 */ b,
		c
	);
	new C5(
		/* c1 */
		/* c2
		 */ b,
		c
	);
	obj.m6(
		/* c1 */
		/* c2
		 */ b,
		c
	);

	// An author blank above the multi-line comment survives.
	fn7(
		/* c1 */

		/* c2
		 */ b
	);
	new C8(
		/* c1 */

		/* c2
		 */ b,
		c
	);
	obj.m9(
		/* c1 */

		/* c2
		 */ b
	);
</script>

<script lang="ts">
	// two own-line block comments after = both stay after = (neither is promoted before =)
	const a =
		/* a1 */
		/* a2 */
		1;

	// with a blank line preserved between them
	const b =
		/* b1 */

		/* b2 */
		2;

	// three block comments, all stay after =
	const c =
		/* c1 */
		/* c2 */
		/* c3 */
		3;

	// a glued run whose later member is multiline stays glued (the run is not split)
	const d =
		/* d1 */ /* d2
		 */ 4;

	// a line comment after `=` forces the break; a following own-line block and a
	// glued multiline block stay on consecutive lines (no blank between them)
	const e = // line
		/* e1 */
		/* e2
		 */ 5;

	// a glued block run stays glued even when a line comment forced the break — its
	// later multiline member is not split onto its own line
	const f = // line
		/* f1 */ /* f2
		 */ 6;
</script>

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
</script>

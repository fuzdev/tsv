<script lang="ts">
	// own-line block leading an object-adjacent member — tsv keeps it above the member
	type A = a &
		// k
		{ x: 1 } &
		/* c1 */
		{ y: 2 };

	// a glued PAIR takes the same answer — the run moves as one
	type B = a &
		// k
		{ x: 1 } &
		/* c1 */ /* c2 */
		{ y: 2 };
</script>

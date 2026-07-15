<script lang="ts">
	// Block comment before the `.` of a test-pattern chain — stays there
	describe /* c1 */.only('a', () => {});

	// Block comment after the `.` — stays on that side
	test./* c2 */ skip('b', () => {});

	// The other recognized variants take the same two gaps
	test./* c3 */ fixme('c', () => {});
	it /* c4 */.only('d', () => {});
</script>

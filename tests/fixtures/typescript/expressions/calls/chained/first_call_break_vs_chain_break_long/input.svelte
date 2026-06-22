<script>
	const fn = () => {
		for (const x of arr) {
			// 100 chars: fits on one line
			expect(aaaaaaaaaaaaaaaaaaaaaaaaaaaa(b, c), `${d} eeeeeeeeeeeee fffffffffff ${g}`).toBe(false);

			// 101 chars: break at .toBe( keeping first call args inline
			expect(aaaaaaaaaaaaaaaaaaaaaaaaaaaaa(b, c), `${d} eeeeeeeeeeeee fffffffffff ${g}`).toBe(
				false
			);
		}
	};
</script>

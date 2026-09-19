<script lang="ts">
	// An indentable block on lines of its own above a curried chain too long for its line is a
	// leading own-line comment like any other: the chain takes its default shape below it.
	const a =
		/**
		 * c
		 */
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
			(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
			(cccccccccccccccccccccccccccccccccc) =>
				d;

	class C {
		p =
			/**
			 * c
			 */
			(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
				(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
				(cccccccccccccccccccccccccccccccccc) =>
					d;
	}
</script>

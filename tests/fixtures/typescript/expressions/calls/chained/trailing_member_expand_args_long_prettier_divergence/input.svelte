<script lang="ts">
	function fn() {
		// fits: the chain stays inline
		assert(!Schema.safeParse({ aaaa: 1, bbbb: [], cccc: { dddd: 2 }, eeee: 3, ffff: 4 }).success);

		// breaks: tsv expands the object arg, keeping .success on the closing brace
		assert(
			!Schema.safeParse({
				aaaa: 1,
				bbbbbbbb: [],
				cccccccc: { dddd: 2 },
				eeeeeeee: 3,
				ffffffff: 4
			}).success
		);
	}
</script>

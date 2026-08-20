<script lang="ts">
	function fn() {
		// fits: the whole chain stays on one line
		assert(!Schema.safeParse({ aaaa: 1, bbbb: [], cccc: { dddd: 2 }, eeee: 3, ffff: 4 }).success);

		// breaks: the arguments stay inline and the trailing member drops to its own line
		assert(
			!Schema.safeParse({ aaaa: 1, bbbbbbbb: [], cccccccc: { dddd: 2 }, eeeeeeee: 3, ffffffff: 4 })
				.success
		);
	}
</script>

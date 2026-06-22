<script lang="ts">
	// Short union value: redundant parens stripped, stays inline
	type ShortU = { [K in T]: A | B };

	// Long union value: parens stripped, breaks to leading-pipe form
	type LongU = {
		[K in T]:
			| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			| Ccccccccccccccccccccccccccccccccccccccccccc;
	};

	// Short intersection value: redundant parens stripped, stays inline
	type ShortI = { [K in T]: A & B };

	// Long intersection value: parens stripped, first member hugs, rest wrap
	type LongI = {
		[K in T]: Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
			Ccccccccccccccccccccccccccccccccccccccccccc;
	};
</script>

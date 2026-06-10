<script lang="ts">
	// Short union value: redundant parens stripped, stays inline
	interface ShortU {
		[k: string]: A | B;
	}

	// Long union value: parens stripped, breaks to leading-pipe form
	interface LongU {
		[k: string]:
			| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			| Ccccccccccccccccccccccccccccccccccccccccccc;
	}

	// Short intersection value: redundant parens stripped, stays inline
	interface ShortI {
		[k: string]: A & B;
	}

	// Long intersection value: parens stripped, first member hugs, rest wrap
	interface LongI {
		[k: string]: Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
			Ccccccccccccccccccccccccccccccccccccccccccc;
	}
</script>

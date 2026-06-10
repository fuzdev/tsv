<script lang="ts">
	// Short union return: redundant parens stripped, stays inline
	type ShortU = () => A | B;

	// Long union return: parens stripped, breaks to leading-pipe form
	type LongU = () =>
		| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		| Ccccccccccccccccccccccccccccccccccccccccccc;

	// Short intersection return: redundant parens stripped, stays inline
	type ShortI = () => A & B;

	// Long intersection return: parens stripped, first member hugs, rest wrap
	type LongI = () => Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
		Ccccccccccccccccccccccccccccccccccccccccccc;
</script>

<script lang="ts">
	// Union check: redundant parens stripped
	type ShortCheckU = A | B extends XEnd ? aaaaa : bbbbb;
	type LongCheckU =
		| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		| Ccccccccccccccccccccccccccccccccccccccccccc extends XEnd
		? aaaaa
		: bbbbb;
	// Union branches: redundant parens stripped, branch hangs
	type LongFalseU = XEnd extends YEnd
		? aaaaa
		: | Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			| Ccccccccccccccccccccccccccccccccccccccccccc;
	type LongTrueU = XEnd extends YEnd
		? | Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			| Ccccccccccccccccccccccccccccccccccccccccccc
		: bbbbb;
	// Intersection check: redundant parens stripped
	type ShortCheckI = A & B extends XEnd ? aaaaa : bbbbb;
	type LongCheckI = Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
		Ccccccccccccccccccccccccccccccccccccccccccc extends XEnd
		? aaaaa
		: bbbbb;
	// Intersection branches: redundant parens stripped, branch hangs
	type LongFalseI = XEnd extends YEnd
		? aaaaa
		: Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
				Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
				Ccccccccccccccccccccccccccccccccccccccccccc;
	type LongTrueI = XEnd extends YEnd
		? Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
				Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
				Ccccccccccccccccccccccccccccccccccccccccccc
		: bbbbb;
</script>

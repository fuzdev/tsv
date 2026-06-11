<script lang="ts">
	// Block comment after ?: intersection branch line at exactly 100 stays inline
	type ShortBlockTrueI = XEnd extends YEnd
		? /* c */ Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa & Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: bbbbb;
	// Block comment after ?: intersection branch line at 101 breaks, continuations hang
	type LongBlockTrueI = XEnd extends YEnd
		? /* c */ Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
				Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: bbbbb;
	// Block comment after :: union branch breaks with leading pipes
	type LongBlockFalseU = XEnd extends YEnd
		? bbbbb
		: /* c */
				| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
				| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
				| Ccccccccccccccccccccccccccccccccccccccccccc;
	// Line comment after ?: short union branch stays inline on the next line
	type ShortLineTrueU = XEnd extends YEnd
		? // c
				Aaaaa | Bbbbb
		: bbbbb;
	// Line comment after ?: intersection branch breaks, continuations hang
	type LongLineTrueI = XEnd extends YEnd
		? // c
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
				Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
				Ccccccccccccccccccccccccccccccccccccccccccc
		: bbbbb;
	// Line comment after :: union branch breaks with leading pipes
	type LongLineFalseU = XEnd extends YEnd
		? bbbbb
		: // c
				| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
				| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
				| Ccccccccccccccccccccccccccccccccccccccccccc;
</script>

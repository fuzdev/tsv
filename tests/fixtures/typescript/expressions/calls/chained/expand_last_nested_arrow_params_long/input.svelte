<script>
	// the hugged argument's own parameters print flat under the expand-last layout; an
	// arrow nested inside its body keeps its break points

	// 100 chars — the nested parameter list stays inline and only the body drops below `=>`
	obj.fn(() =>
		// c
		({
			a: ({ aaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccc }) =>
				a
		})
	);

	// 101 chars — the nested parameter list breaks
	obj.fn(() =>
		// c
		({
			a: ({
				aaaaaaaaaaaaaaaaaaaaaaaaaaa,
				bbbbbbbbbbbbbbbbbbbbbbbbbb,
				cccccccccccccccccccccccccc
			}) => a
		})
	);
</script>

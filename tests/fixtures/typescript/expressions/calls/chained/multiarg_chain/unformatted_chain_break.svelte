<script>
	fn(a.has(b), `text text text text text text text text text text text text text text text text`)
		.method(true);

	// Arrow last-arg (template body): the head expands, the arrow's params fit so
	// they stay inline, and only the body breaks onto its own line.
	const a = Array.from({ length: n }, (a, b) => `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ${b}`)
		.join('\n');

	// Arrow last-arg (binary body): params stay inline, the binary breaks under the =>.
	const b = Array.from({ length: n }, (a, b) => aaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbb + ccccccccccccccccc + ddddddddddddddddd + eeeeeeeeeeeeeeeee)
		.join('\n');

	// Contrast: the arrow's own param list overflows even on its own line, so it
	// breaks — fitting params stay inline without suppressing a genuine break.
	const c = Array.from(config, (aaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbb, cccccccccccccccccc, dddddddddddddddddd, eeeeeeeeeeeeeeeeee) => x)
		.join('\n');
</script>

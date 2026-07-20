<script lang="ts">
	// Arrow with a call body (last-arg hug), compact source - stays inline (atomized)
	const a = `text1 text2 text3 text4 text5 text6 text7 text8 text9 ${items.map((i) => fn(i)).join(', ')}`;

	// Arrow with a call body, no chain - same hug path
	const b = `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 ${items.map((i) => fn(i))}`;

	// Object literal as last argument - hug path
	const c = `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10xx ${fn(a, { b: c, d: e })}`;

	// Array literal as last argument - hug path
	const d = `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 ${fn(a, [b, c, d])}`;

	// Contrast: plain arguments, no hug - already inline
	const e = `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10xxx ${fn(aaaa, bbbb, cccc)}`;

	// Contrast: arrow with a member body, no hug - already inline
	const f = `text1 text2 text3 text4 text5 text6 text7 text8 text9xxxxx ${items.map((i) => i.value)}`;

	// Boundary: exactly 100 chars - fits, stays inline
	const g = `text1 text2 text3 text4 text5 text6 text7 text8 text9 te10 ${items.map((i) => fn(i))}`;

	// Boundary: exactly 101 chars - over width, still inline (atomized)
	const h = `text1 text2 text3 text4 text5 text6 text7 text8 text9 tex10 ${items.map((i) => fn(i))}`;

	// Contrast: a source newline INSIDE the interpolation - not atomized, so the call breaks
	const i = `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 ${items.map((i) =>
		fn(i)
	)}`;
</script>

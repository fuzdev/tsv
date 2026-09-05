<script lang="ts">
	// An interpolation whose SOURCE spans lines is not atomized: `${` hugs (a CallExpression is
	// non-qualifying) and the call breaks its own arguments, exactly as prettier does.

	// Compact source: every interpolation is atomized, so the line stays inline past width.
	const a = `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 ${ fn( 'aaa' ) }, ${ fn( 'bbb' ) }, ${ fn( 'ccc' ) }`;

	// Source newline in the middle interpolation: that call breaks its lone short argument
	// while its compact neighbors stay inline.
	const b = `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 ${fn('aaa')}, ${fn(
		'bbb'
	)}, ${ fn( 'ccc' ) }`;

	// Boundary: exactly 100 chars flat - fits, so the source newline collapses.
	const c = `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 te12 ${fn(
			'aaa'
		)}`;

	// Boundary: exactly 101 chars flat - over width, so the call breaks.
	const d = `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 tex12 ${fn(
		'aaa'
	)}`;
</script>

<script lang="ts">
	// after-comma block hugs the options arg → leads it
	const a = import('./a',/* c */b);

	// before-comma block trails the source arg
	const c = import('./a'/* c */,d);

	// line comment after the comma forces multiline; stays after the comma
	const e = import('./a',// c
g);

	// own-line block between comma and options leads the options arg
	const i = import('./a',
/* c */
j);

	// long source forces a break; the hugging block leads the (flat) options object
	const h = import('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',/* c */{with:{type:'json'}});

	// a long options object expands; the hugging block leads it
	const k = import('./a',/* c */{with:{type:'json'},aaaaaaaaaaaaaaaaaaaaaa:bbbbbbbbbbbbbbbbbbbbbbbbbbbbb,ccccccccccccc:ddddddddddddddddd});
</script>

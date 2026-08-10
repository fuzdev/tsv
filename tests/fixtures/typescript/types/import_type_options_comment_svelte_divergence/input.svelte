<script lang="ts">
	// A comment in an import type's argument gaps — specifier→options and options→`)`. A
	// block before the comma trails the specifier; one after it leads the options inline. An
	// own-line comment after the options, and a line comment in the gap, open the parens; an
	// author blank line between the arguments is preserved.
	type T = import('./a' /* c */, { with: { t: 'json' } });

	type U = import('./b', /* c */ { with: { t: 'json' } });

	type V = import('./c', { with: { t: 'json' } } /* c */);

	type W = import(
		'./d',
		{ with: { t: 'json' } }
		/* c */
	);

	type X = import(
		'./e', // c
		{ with: { t: 'json' } }
	);

	type Y = import(
		'./f',

		{ with: { t: 'json' } }
	);
</script>

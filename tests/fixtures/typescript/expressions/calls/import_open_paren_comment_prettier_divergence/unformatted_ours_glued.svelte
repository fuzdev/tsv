<script lang="ts">
	// line comment trailing `(` — sole argument
	const a = import(// c1
		'./a'
	);

	// line comment trailing `(` — with an options argument
	const b = import(// c2
		'./b',
		{ with: { type: 'json' } }
	);

	// block comment trailing `(`, then an own-line leading comment
	const c = import(/* paren */
		// lead
		'./c'
	);

	// a run that begins BEFORE the `(` is the head's and stays whole below it —
	// both formatters agree here, so this is the null control for the pull
	const e = import/* pre */(// post
		'./e'
	);

	// the import TYPE shares the argument layout, so it answers the gap identically
	type A = import(// c4
		'./f'
	).B;
</script>

<script lang="ts">
	// an own-line directive before the source argument freezes it
		const a = import(
		// prettier-ignore
		base  +  './a'
	);

	// with an options argument too
		const b = import(
		// prettier-ignore
		base  +  './a'  ,
			opts  
	);

	// an own-line directive in the source→options gap freezes the options argument
		const c = import(
			'./a',
		// prettier-ignore
		{ with: { type:   'json' } }
	);

	// an own-line block spelling freezes identically
		const d = import(
			'./a',
		/* prettier-ignore */
		{ with: { type:   'json' } }
	);
</script>

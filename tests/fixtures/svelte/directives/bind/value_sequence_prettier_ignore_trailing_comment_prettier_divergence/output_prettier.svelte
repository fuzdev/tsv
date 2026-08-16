<!-- a comment PAST the frozen slice is outside it: the freeze replaces the sequence's own
	doc, not the gap between that doc's end and the value's `}` -->
<input
	bind:value={
		// prettier-ignore
		(()  =>  a,
		(v)  =>  (a  =  v))
	}
/>

<!-- a trailing line comment ends the line, and the `}` reuses its break -->
<input
	bind:value={
		// prettier-ignore
		(()  =>  a,
		(v)  =>  (a  =  v))
	}
/>

<!-- a comment INSIDE the slice rides in the verbatim text, spacing and all -->
<input
	bind:value={
		// prettier-ignore
		(()  =>  a, /* c */
		(v)  =>  (a  =  v))
	}
/>

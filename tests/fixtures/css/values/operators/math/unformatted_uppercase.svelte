<style>
	div {
		/* `*` is always spaced, however it was authored */
		margin: 1.5 * 2.5;
		padding: 1.5 * 2.5;
		width: aa * bb;
		/* `+` keeps a glued pair glued, and spaces one whose neighbour is a word */
		height: 1.5+2.5;
		top: 1.5 + 2.5;
		right: aa + bb;
		/* a run that starts with an ident character absorbs `-`, so `-` never
		opens a gap of its own */
		bottom: 1.5-2.5;
		left: aa-bb;
		/* `+` ends the run, so the hex colour before it is a colour and folds;
		`-` does not, so the same run stays one opaque token */
		flex: #FFF + 5;
		order: #FFF-5;
		/* inside calc() a glued `+` or `-` is kept as authored, where `*` is
		still spaced */
		z-index: calc(1.5+2.5);
		gap: calc(1.5 + 2.5);
		inset: calc(1.5 * 2.5);
	}
</style>

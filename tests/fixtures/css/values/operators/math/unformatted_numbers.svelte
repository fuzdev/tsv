<style>
	div {
		/* `*` is always spaced, however it was authored */
		margin: 1.50 * 2.50;
		padding: 1.50 * 2.50;
		width: aa * bb;
		/* `+` keeps a glued pair glued, and spaces one whose neighbour is a word */
		height: 1.50+2.50;
		top: 1.50 + 2.50;
		right: aa + bb;
		/* a run that starts with an ident character absorbs `-`, so `-` never
		opens a gap of its own */
		bottom: 1.50-2.50;
		left: aa-bb;
		/* `+` ends the run, so the hex colour before it is a colour and folds;
		`-` does not, so the same run stays one opaque token */
		flex: #fff + 5;
		order: #FFF-5;
		/* inside calc() a glued `+` or `-` is kept as authored, where `*` is
		still spaced */
		z-index: calc(1.50+2.50);
		gap: calc(1.50 + 2.50);
		inset: calc(1.50 * 2.50);
	}
</style>

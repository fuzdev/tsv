<style>
	div {
		/* Over-arity space form: too many channels and no slash — not a valid color,
		   so it round-trips verbatim like any function (never reconstructed with
		   commas or with a channel dropped). */
		color: rgb(100%	0%  none   5	50%);
		background: hsl(180   50%	50%  0.5);
		/* Over-arity comma form: the extra part is preserved, not dropped. */
		border-color: rgb(1 , 2 ,3,  4 , 5);
		/* Slash-alpha form with too few channels: the slash alpha separator is kept
		   verbatim, never collapsed into a channel. */
		outline-color: hsl(180	none   /   0%);
	}
</style>

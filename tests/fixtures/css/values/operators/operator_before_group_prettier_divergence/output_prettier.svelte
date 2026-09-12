<style>
	div {
		/* an operator's authored glue against a following group is kept, the same
		answer the glue AFTER a group already gets */
		margin: 1.5/ (2.5);
		padding: 1.5+ (2.5);
		width: 1.5- (2.5);
		height: (1.5)/ (2.5);
		/* inside calc() too, where a `+` or `-` gap is the difference between a
		valid expression and an invalid one */
		top: calc(1.5+ (2.5));
		/* the same authorings with the gap written out are untouched, and so is
		an operator glued on the group's other side */
		right: 1.5 / (2.5);
		bottom: calc(1.5 + (2.5));
		left: (1.5)/2.5;
	}
</style>

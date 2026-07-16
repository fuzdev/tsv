<style>
	div {
		/* an escape INSIDE the url content: prettier keeps it */
		border-image: url(a\ b);

		/* the same escape at the END, where the payload abuts the closing paren */
		background: url(x\);
	}
</style>

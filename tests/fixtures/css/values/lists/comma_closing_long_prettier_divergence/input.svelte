<style>
	div {
		/* a comment routes the value to the source-extracting path; at 100 columns it stays inline */
		background: linear-gradient(/* k */ #000 0%, #111 2%, #222 30%, #333 50%, #444 90%, #555 100%,);
		/* one column more (101) breaks those arguments one per line, closing comma on the last */
		background: linear-gradient(
			/* k */ #000 0%,
			#111 2%,
			#222 30%,
			#333 50%,
			#444 90%,
			#5555 100%,
		);
		/* a broken list reserves the `,;` its last element takes, so at 100 the element stays flat */
		transition:
			a 1s,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 1s ccccccccccccccccccccccccccccccccccccccccccccc,;
		/* one column more (101) and that element wraps at its space rather than overrunning */
		transition:
			a 1s,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 1s
				cccccccccccccccccccccccccccccccccccccccccccccc,;
	}
</style>

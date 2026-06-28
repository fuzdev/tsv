<style>
	/* a comment between conditions stays in place */
	@supports  ( display: grid )   /* note */   and  ( gap: 1px )  {
		.a  {
			color:  red;
		}
	}

	/* a comment whose text contains the `and` connector word stays put */
	@supports  ( display: grid )   /* x and y */   and  ( gap: 1px )  {
		.b  {
			color:  blue;
		}
	}

	/* the same when the comment contains the `or` connector word */
	@supports  ( display: grid )   /* p or q */   or  ( gap: 1px )  {
		.c  {
			color:  green;
		}
	}
</style>

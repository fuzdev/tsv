<style>
	/* a comment among a run's members stays where it was written, glued where it was glued */
	a[b] /* c */, c {
		color: red;
	}
	a,  /* c */ b {
		color: red;
	}
	a, /* c */ ﻿ b {
		color: red;
	}
	:is(a,  /* c */ b) {
		color: red;
	}
	:is( /* c */ a) {
		color: red;
	}
</style>

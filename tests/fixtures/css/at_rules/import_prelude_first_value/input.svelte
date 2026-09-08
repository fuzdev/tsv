<style>
	/* a prelude that leads with no url()/string is value-read like the rest of `@import`:
	a number normalizes, a hex colour lowercases, at the first position as after a url */
	@import 1e3;
	@import #fff;
	@import 0.5px;
	@import foo 1e3;
	@import (a: 0.5px);
	@import /* c */ 1e3;
</style>

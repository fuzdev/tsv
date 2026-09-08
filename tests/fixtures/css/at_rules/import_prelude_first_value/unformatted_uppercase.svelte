<style>
	/* a prelude that leads with no url()/string is value-read like the rest of `@import`:
	a number normalizes, a hex colour lowercases, at the first position as after a url */
	@import 1E3;
	@import #FFF;
	@import 0.5PX;
	@import foo 1E3;
	@import (a: 0.5PX);
	@import /* c */ 1E3;
</style>

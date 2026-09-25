<style>
	/* a boundary run in an import media query list stays where it was written */
	@import 'a.css'   screen, print;
	@import 'a.css' screen  , print;
	@import 'a.css' screen, ﻿ print;
</style>

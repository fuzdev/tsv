<style>
	/* a boundary run at the head or tail of a condition, or after its not, stays where it was written */
	@supports   (a: b) {
		c {
			color: red;
		}
	}
	@supports not   (a: b) {
		c {
			color: red;
		}
	}
	@supports (a: b) ﻿ {
		c {
			color: red;
		}
	}
	@container (a: b)   {
		c {
			color: red;
		}
	}
</style>

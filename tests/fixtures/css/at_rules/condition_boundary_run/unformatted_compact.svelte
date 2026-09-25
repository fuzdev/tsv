<style>
	/* a boundary run between the parts of a condition stays where it was written */
	@supports (a:b) and ﻿ (c:d) {
		e {
			color: red;
		}
	}
	@supports (a:b) or   (c:d) {
		e {
			color: red;
		}
	}
	@supports (a:b)   and (c:d) {
		e {
			color: red;
		}
	}
	@supports (a:b) and  (c:d) {
		e {
			color: red;
		}
	}
	@supports selector(a  ) {
		e {
			color: red;
		}
	}
	@container x   (a: b) {
		e {
			color: red;
		}
	}
	@container (a: b) and ﻿ (c: d) {
		e {
			color: red;
		}
	}
</style>

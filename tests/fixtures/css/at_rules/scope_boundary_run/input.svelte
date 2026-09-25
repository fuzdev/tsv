<style>
	/* a boundary run in a scope prelude stays where it was written */
	@scope (.a  ) {
		b {
			color: red;
		}
	}
	@scope (.a) to (.b ﻿) {
		b {
			color: red;
		}
	}
	@scope (.a /* c */ ) {
		b {
			color: red;
		}
	}
	@scope (.a)   to (.b) {
		b {
			color: red;
		}
	}
	@scope (.a) to ﻿ (.b) {
		b {
			color: red;
		}
	}
	@scope   {
		b {
			color: red;
		}
	}
</style>

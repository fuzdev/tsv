<div>
	<style>
		b {
			color: red;
		}
		﻿		a {
			color: red;
		}
	</style>
</div>

<style>
	/* each ASCII whitespace stretch in a boundary run prints as one space */
	b {
		color: red;
	}
	﻿		a {
		color: red;
	}
	 		a {
		color: red;
	}
	 		 a {
		color: red;
	}
	﻿		﻿		a {
		color: red;
	}
	a,
	﻿		b {
		color: red;
	}
	a > ﻿		b {
		color: red;
	}
	:is(﻿		b) {
		color: red;
	}
	﻿		@media screen {
		a {
			color: red;
		}
	}
	@media screen {
		 		a {
			color: red;
		}
	}
</style>

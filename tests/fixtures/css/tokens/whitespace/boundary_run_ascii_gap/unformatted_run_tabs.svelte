<style>
	/* each ASCII whitespace stretch beside a boundary run prints as one space */
	:is(a		 		 ) {
		color: red;
	}
	a		 		 ,
	b {
		color: red;
	}
	::part(a		 		 b) {
		color: red;
	}
	a		﻿		{
		color: red;
	}
	a		 		 		{
		color: red;
	}
	:nth-child(2n		 ) {
		color: red;
	}
	:nth-child(2n		 		 ) {
		color: red;
	}
</style>

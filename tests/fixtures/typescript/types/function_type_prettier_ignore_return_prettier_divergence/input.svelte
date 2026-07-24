<script lang="ts">
	// an own-line directive after `=>` stays own-line and freezes the return type
	type A = () =>
		// prettier-ignore
		{x:   1};

	// constructor-type host
	type B = new () =>
		// prettier-ignore
		{x:   2};

	// abstract-constructor-type host
	type C = abstract new () =>
		// prettier-ignore
		{x:   3};

	// control: a plain comment keeps the trailing relocation
	type D = () => // c
	{ x: 1 };

	// placement, not spelling: a BLOCK directive alone on its line in the `)`→`=>` gap
	type E = ()
	/* prettier-ignore */
	=> {x:   4};
</script>

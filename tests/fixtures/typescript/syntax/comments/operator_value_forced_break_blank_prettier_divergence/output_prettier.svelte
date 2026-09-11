<script lang="ts">
	import c from './c' with {
		type /* x */:
			// y
			'json'
	};

	// A block comment on the operator's line whose break is forced by a line comment below it
	// keeps the author blank after it.

	// declarator `=`
	const a1 =
		/* x */

		// y
		b;

	// assignment `=`
	a2 =
		/* x */

		// y
		b;

	// enum member `=`
	enum E {
		A /* x */ = // y
		1
	}

	// for-loop init
	for (
		let i =
			/* x */

			// y
			0;
		;
	) {}

	// object property `:`
	x3 = {
		/* x */

		k:
			// y
			b
	};
</script>

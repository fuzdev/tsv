<script lang="ts">
	// A multi-line block the author broke after hangs the mapped-type value and keeps
	// the authored break.
	type A = {
		[K in string]:
			/* c
d */
			Ooo | Ppp;
	};

	// The indentable kind answers the same way.
	type B = {
		[K in string]:
			/**
			 * c
			 */
			Ooo | Ppp;
	};

	// A single-line block the author gave its own line hangs the value too.
	type C = {
		[K in string]:
			/* c */
			Ooo | Ppp;
	};

	// Null control: a multi-line block GLUED to the value keeps that line, the pair
	// hanging together below the `:`.
	type D = {
		[K in string]:
			/* c
d */ Ooo | Ppp;
	};

	// Null control: a single-line block glued after the `:` stays inline.
	type E = {
		[K in string]: /* c */ Ooo | Ppp;
	};

	// An author blank survives with the break it belongs to — after the run...
	type G = {
		[K in string]:
			/* c
d */

			Ooo | Ppp;
	};

	// ...and between two comments of one run.
	type H = {
		[K in string]:
			/* c1 */

			/* c2 */
			Ooo | Ppp;
	};

	// Null control: no comment.
	type F = {
		[K in string]: Ooo | Ppp;
	};

	export type { A, B, C, D, E, F, G, H };
</script>

<style>
	/* after the opening bracket: glued to `[`, a single space to the name */
	[  /* c1 */  attr  ] {
		color: red;
	}

	/* between the name and the matcher */
	[attr  /* c2 */  =  'value'] {
		color: blue;
	}

	/* between the matcher and the value */
	[attr  =  /* c3 */  'value'] {
		color: green;
	}

	/* between the value and the closing bracket */
	[attr  =  'value'  /* c4 */  ] {
		color: teal;
	}

	/* between the value and the case flag */
	[attr  =  'value'  /* c5 */  i  ] {
		color: navy;
	}

	/* between the case flag and the closing bracket */
	[attr  =  'value'  i  /* c6 */  ] {
		color: olive;
	}

	/* a bare presence selector's one interior gap */
	[attr  /* c7 */  ] {
		color: purple;
	}

	/* two comments in one gap */
	[attr  /* c8 */  /* c9 */  =  'value'] {
		color: orange;
	}

	/* a bracket inside the comment content is not the closing bracket */
	[attr  /* ] */  =  'value'] {
		color: gray;
	}

	/* the control: selectors-4 forbids white space between the components of a
	   <wq-name> or an <attr-matcher>, so a comment there stays glued */
	[svg/* c10 */|attr] {
		color: aqua;
	}

	[svg|/* c11 */attr] {
		color: maroon;
	}

	[*/* c12 */|attr] {
		color: fuchsia;
	}

	[|/* c13 */attr] {
		color: lime;
	}

	[attr~/* c14 */='value'] {
		color: silver;
	}

	[attr|/* c15 */='value'] {
		color: black;
	}
</style>

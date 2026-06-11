<style>
	.class1 {
		/* type/ident attached to & forms one compound selector (no descendant space) */
		&__a {
			color: red;
		}
		&div {
			color: blue;
		}
		div& {
			color: green;
		}
		*& {
			color: orange;
		}
		&& {
			color: purple;
		}
		a&b {
			color: olive;
		}
		/* compound continues past the attached ident */
		&__a.class2:hover {
			margin: 0;
		}
		/* whitespace keeps a real descendant combinator */
		& div {
			padding: 0;
		}
		/* compound with class and pseudo-class */
		&.class3 {
			margin: 0;
		}
		&:hover {
			display: block;
		}
	}
</style>

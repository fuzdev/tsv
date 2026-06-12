<style>
	/* Basic scoping */
	@scope ( .class1 ) {
		img {
			border: 1px solid;
		}
	}

	/* Scope with limit */
	@scope ( .class1 )  to  ( .class2 ) {
		p {
			color: blue;
		}
	}

	/* Named scope root */
	@scope ( #id1 ) {
		h1 {
			font-size: 2rem;
		}
	}

	/* Scope with multiple selectors */
	@scope ( .class1 ,  .class3 ) {
		.class4 {
			font-weight: bold;
		}
	}

	/* Scope with complex limit */
	@scope ( .class5 )  to  ( .class6 ,  .class7 ) {
		a {
			text-decoration: underline;
		}
	}

	/* Nested scopes */
	@scope ( .outer ) {
		.outer-content {
			padding: 1rem;
		}

		@scope ( .inner ) {
			span {
				font-weight: bold;
			}
		}
	}

	/* Scope with descendant selectors */
	@scope ( article  >  header ) {
		h2 {
			margin: 0;
		}
	}

	/* Scope with pseudo-class */
	@scope ( .class8:hover ) {
		.class9 {
			display: block;
		}
	}

	/* Scope with limit and nested rules */
	@scope ( .class10 )  to  ( .class11 ) {
		section {
			margin-bottom: 2rem;
		}

		.class12 {
			position: sticky;
		}
	}

	/* Scope with attribute selector */
	@scope ( [data-theme='dark'] ) {
		.background {
			background: black;
			color: white;
		}
	}
</style>

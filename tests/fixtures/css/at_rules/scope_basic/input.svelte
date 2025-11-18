<style>
	/* Basic scoping */
	@scope (.card) {
		img {
			border: 1px solid;
		}
	}

	/* Scope with limit */
	@scope (.card) to (.ignore) {
		p {
			color: blue;
		}
	}

	/* Multiple selectors */
	@scope (.card, .panel) {
		.title {
			font-weight: bold;
		}
	}

	/* Complex limit */
	@scope (.container) to (.skip, .exclude) {
		a {
			text-decoration: underline;
		}
	}

	/* Nested scopes */
	@scope (.outer) {
		.outer-content {
			padding: 1rem;
		}

		@scope (.inner) {
			span {
				font-weight: bold;
			}
		}
	}
</style>

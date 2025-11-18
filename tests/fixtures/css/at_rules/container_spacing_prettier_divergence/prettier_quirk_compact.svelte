<style>
	/* Basic container query */
	@container (min-width:700px) {
		.card {
			display: grid;
		}
	}

	/* Named container */
	@container sidebar (min-width:400px) {
		.widget {
			flex-direction: row;
		}
	}

	/* Boolean operators - and */
	@container (min-width:700px)and(max-width:1000px) {
		.content {
			columns: 2;
		}
	}

	/* Boolean operators - or */
	@container (min-width:700px)or(orientation:portrait) {
		.banner {
			padding: 1rem;
		}
	}

	/* Boolean operators - not */
	@container not(min-width:400px) {
		.mobile {
			display: block;
		}
	}

	/* Style queries */
	@container style(--theme:dark) {
		.card {
			background: black;
			color: white;
		}
	}

	/* Named container with style query */
	@container sidebar style(--highlight:true) {
		strong {
			color: violet;
		}
	}
</style>

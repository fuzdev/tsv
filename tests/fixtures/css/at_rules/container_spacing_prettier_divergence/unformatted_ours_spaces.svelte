<style>
	/* Basic container query */
	@container   (  min-width  :   700px  )   {
		.class1 {
			display: grid;
		}
	}

	/* Named container */
	@container   name1   (  min-width  :   400px  )  {
		.class2 {
			flex-direction: row;
		}
	}

	/* Boolean operators - and */
	@container (min-width: 700px)   and   (max-width: 1000px) {
		.class3 {
			columns: 2;
		}
	}

	/* Boolean operators - or */
	@container (min-width: 700px)   or   (orientation: portrait) {
		.class4 {
			padding: 1rem;
		}
	}

	/* Boolean operators - not */
	@container   not   (min-width: 400px) {
		.class5 {
			display: block;
		}
	}

	/* Style queries */
	@container   style(  --theme  :  dark  )  {
		.class1 {
			background: black;
			color: white;
		}
	}

	/* Named container with style query */
	@container   name1   style(  --highlight  :  true  )  {
		strong {
			color: violet;
		}
	}
</style>

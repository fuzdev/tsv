<style>
	div {
		/* Basic calc() expressions */
		width: calc(
			100%
			-
			2rem
		);
		height: calc(
			50vh
			+
			10px
		);
		margin: calc(
			1em
			*
			2
		);
		padding: calc(
			20px
			/
			4
		);
		/* Nested calc() */
		border: calc(
			calc(
				10px
				+
				5px
			)
			*
			2
		);
		outline: calc(
			100%
			-
			calc(
				2rem
				+
				10px
			)
		);
		/* Mixed units */
		top: calc(
			50vh
			-
			10px
		);
		left: calc(
			1em
			+
			2px
		);
		right: calc(
			100%
			/
			3
			-
			1rem
		);
		/* Complex expressions */
		min-width: calc(
			(
				100%
				-
				40px
			)
			/
			2
		);
		min-height: calc(
			100vh
			-
			60px
			-
			2rem
		);
		max-width: calc(
			1em
			+
			0.5rem
			+
			2px
		);
		/* Zero and negative values */
		z-index: calc(
			100%
			-
			0
		);
		order: calc(
			0px
			+
			50vh
		);
		flex: calc(
			-10px
			+
			20px
		);
		opacity: calc(
			100%
			+
			-5px
		);
	}
</style>

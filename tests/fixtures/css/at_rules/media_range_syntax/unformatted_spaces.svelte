<style>
	/* Single comparison operators (MQ Level 4) */
	@media (  width   >=    768px  ) {
		.desktop {
			font-size: 16px;
		}
	}

	@media (  width   >    600px  ) {
		.medium {
			padding: 2rem;
		}
	}

	@media (  width   <=    1200px  ) {
		.constrained {
			max-width: 1200px;
		}
	}

	@media (  width   <    1400px  ) {
		.narrow {
			columns: 1;
		}
	}

	/* Range feature names */
	@media (  height   >=    600px  ) {
		.tall {
			min-height: 100vh;
		}
	}

	@media (  resolution   >=    2dppx  ) {
		.retina {
			image-rendering: crisp-edges;
		}
	}

	/* Double comparison (range syntax) */
	@media (  400px   <=    width    <=     1200px  ) {
		.range {
			padding: 1rem;
		}
	}

	/* Combined with boolean operators */
	@media (width >= 768px)   and   (width <= 1024px) {
		.tablet {
			display: flex;
		}
	}

	@media (width > 1200px)   or   (orientation: landscape) {
		.wide {
			grid-template-columns: repeat(3, 1fr);
		}
	}
</style>

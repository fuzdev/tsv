<style>
	@supports (display: grid) and (gap: 1rem) {
		.grid-with-gap {
			display: grid;
			gap: 1rem;
		}
	}

	@supports (display: flex) or (display: grid) {
		.flex-or-grid {
			display: flex;
		}
	}

	@supports not (display: grid) {
		.no-grid {
			display: block;
		}
	}

	@supports (display: grid) and ((gap: 1rem) or (grid-gap: 1rem)) {
		.nested-logic {
			display: grid;
		}
	}

	@supports ((display: flex) or (display: grid)) and (color: oklch(0.5 0.2 180)) {
		.mixed-complex {
			color: oklch(0.5 0.2 180);
		}
	}
</style>

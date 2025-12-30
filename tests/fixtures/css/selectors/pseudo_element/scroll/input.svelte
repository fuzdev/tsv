<style>
	/* Scroll marker styling */
	::scroll-marker {
		background: blue;
	}

	/* Scroll marker group */
	::scroll-marker-group {
		padding: 10px;
	}

	/* Scroll button with direction */
	::scroll-button(up) {
		border-radius: 50%;
	}

	::scroll-button(down) {
		border-radius: 50%;
	}

	::scroll-button(left) {
		border-radius: 50%;
	}

	::scroll-button(right) {
		border-radius: 50%;
	}

	/* Combined with selectors */
	.class1::scroll-marker {
		color: red;
	}

	/* With pseudo-class */
	.class1:hover::scroll-marker-group {
		opacity: 0.8;
	}
</style>

<style>
	/* Basic view transition root */
	::view-transition {
		position: fixed;
		inset: 0;
	}

	/* View transition group with identifier */
	::view-transition-group(root) {
		animation-duration: 0.5s;
	}

	/* View transition group with wildcard */
	::view-transition-group(*) {
		animation-timing-function: ease-in-out;
	}

	/* Image pair for transitions */
	::view-transition-image-pair(root) {
		isolation: isolate;
	}

	/* Old image state */
	::view-transition-old(root) {
		animation: fade-out 0.3s;
	}

	/* New image state */
	::view-transition-new(root) {
		animation: fade-in 0.3s;
	}

	/* With type selector */
	.page::view-transition-group(content) {
		animation-duration: 0.25s;
	}

	/* Nested pseudo-elements */
	::view-transition-group(sidebar)::view-transition-old(*) {
		opacity: 0;
	}

	/* With pseudo-class */
	.animated:hover::view-transition-new(element) {
		transform: scale(1.1);
	}
</style>

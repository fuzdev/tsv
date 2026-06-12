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
	.class1::view-transition-group(content) {
		animation-duration: 0.25s;
	}

	/* Nested pseudo-elements */
	::view-transition-group(name1)::view-transition-old(*) {
		opacity: 0;
	}

	/* With pseudo-class */
	.class2:hover::view-transition-new(name2) {
		transform: scale(1.1);
	}
</style>

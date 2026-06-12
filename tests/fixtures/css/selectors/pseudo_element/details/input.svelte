<style>
	/* Details content styling */
	details::details-content {
		padding: 1rem;
	}

	/* Column pseudo-element */
	.multi-column::column {
		background: lightgray;
	}

	/* Combined with type selector */
	details::details-content {
		margin: 0.5rem;
	}

	/* With pseudo-class */
	details:open::details-content {
		animation: slide-down 0.3s;
	}

	/* Column with class selector */
	.class1::column {
		border-right: 1px solid gray;
	}
</style>

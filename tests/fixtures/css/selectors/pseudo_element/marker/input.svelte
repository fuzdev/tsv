<style>
	::marker {
		color: red;
	}

	li::marker {
		font-size: 1rem;
	}

	:is(::marker, ::before) {
		content: '';
	}
</style>

<style>
	.class:is(:not(:has(.foo))) {
	}
	.class:where(:is(.a, .b):not(.c)) {
	}
	.class:is(:is(:is(.a))) {
	}
</style>

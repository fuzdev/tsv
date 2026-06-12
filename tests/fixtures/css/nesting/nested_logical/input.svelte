<style>
	.class1:is(:not(:has(.class2))) {
	}
	.class1:where(:is(.a, .b):not(.c)) {
	}
	.class1:is(:is(:is(.a))) {
	}
</style>

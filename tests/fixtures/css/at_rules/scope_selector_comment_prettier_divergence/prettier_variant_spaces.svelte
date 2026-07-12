<style>
	/* comment leading the root, trailing the limit */
	@scope (  /* c1 */  .class1) to (.class2  /* c2 */  ) {
		color: red;
	}

	/* comment trailing the root, leading the limit */
	@scope (.class3  /* c3 */  ) to (  /* c4 */  .class4) {
		color: blue;
	}
</style>

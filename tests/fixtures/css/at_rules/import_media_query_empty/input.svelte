<style>
	/* an empty query between two others keeps both commas */
	@import url('./a.css') screen, , print;
	@import url('./b.css') screen, , , print;

	/* empty first query, then an empty interior one */
	@import url('./c.css'), , screen;
	@import url('./d.css') supports(display: grid), , screen;

	/* a comma closing the list is a separator with no entry after it, so it is dropped */
	@import url('./e.css') screen;
</style>

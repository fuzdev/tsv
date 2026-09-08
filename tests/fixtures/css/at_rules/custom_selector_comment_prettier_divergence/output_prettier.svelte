<style>
	/* a comment in the prelude is preserved at every gap, padded by one space on each side:
	leading, after the name, before and after a comma, in a combinator gap, and trailing */
	@custom-selector :--a h1, h2;
	@custom-selector :--b h1, h2;
	@custom-selector :--c h1, h2;
	@custom-selector :--d h1, h2;
	@custom-selector :--e h1 > h2, h3;
	@custom-selector :--f h1, h2;
</style>

<style>
	/* Two layers */
	@layer base, components;

	/* Three layers */
	@layer reset, base, utilities;

	/* Four layers */
	@layer theme, layout, components, utilities;

	/* Layer block */
	@layer base {
		h1 {
			font-size: 2rem;
		}
	}
</style>

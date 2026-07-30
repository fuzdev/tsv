<style>
	/* leading comma: the query list's first query is empty */
	@import url('./a.css'), screen;
	@import './b.css', screen;

	/* the comma glues to whichever prelude component precedes it */
	@import url('./c.css') layer, screen;
	@import url('./d.css') layer(base), screen;
	@import url('./e.css') supports(display: grid), screen;

	/* a comment before the comma keeps the comma glued to it */
	@import url('./f.css') supports(display: grid) /* c */, screen;
</style>

<style>
	/* relative list: bare first item (descendant), then combinator-leading tail items */
	figure:has(img, + figcaption, ~ .aside, > .caption) {
		margin: 0;
	}
</style>

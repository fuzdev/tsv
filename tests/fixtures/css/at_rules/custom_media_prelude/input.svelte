<style>
	/* `@custom-media` is read by the media-query parser, like `@media`: its prelude's
	whitespace collapses, its feature names lowercase and its units fold, while the
	`--` name keeps its case (an author-defined name has no canonical casing) */
	@custom-media --a (max-width: 30em);
	@custom-media --KeepName (min-width: 100px);
	@custom-media --b (min-width: 100px) and (max-width: 200px);
	@custom-media --c not all;
</style>

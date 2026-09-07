<style>
	/* `@custom-media` is read by the media-query parser, like `@media`: its prelude's
	whitespace collapses, its feature names lowercase and its units fold, while the
	`--` name keeps its case (an author-defined name has no canonical casing) */
	@custom-media --a (MAX-WIDTH: 30EM);
	@custom-media --KeepName (MIN-WIDTH: 100PX);
	@custom-media --b (MIN-WIDTH: 100px) and (MAX-WIDTH: 200px);
	@custom-media --c not all;
</style>

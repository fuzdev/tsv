<script>
	// a redundant paren pair around the expression is dropped, so the directive it holds
	// leads the statement and freezes it (the variant carries the authored form)
	(
	// prettier-ignore
	fn  (  aaa  ));

	(
	// prettier-ignore
	bbb  +  ccc);

	// an own-line block comment behaves identically — placement keys the freeze
	(
	/* prettier-ignore */
	ddd  .  eee);
</script>

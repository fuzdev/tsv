<script>
	// an own-line directive in the `(`→expression gap of a statement whose parens are
	// required freezes the expression; the parens stay outside the frozen slice
	// prettier-ignore
	({ aaa:  1 });

	// prettier-ignore
	(class  {});

	// an own-line block comment behaves identically — placement keys the freeze
	/* prettier-ignore */
	(function  ()  {});

	// a slice whose leftmost token needs parens keeps them too — a verbatim slice has no
	// interior for the printer to wrap, so the shell goes around the whole slice
	// prettier-ignore
	{ bbb:  2 }.ccc;
</script>

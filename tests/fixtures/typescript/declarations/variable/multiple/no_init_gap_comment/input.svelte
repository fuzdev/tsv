<script>
	// A gap comment on declarators without initializers keeps its line: the comma trails the
	// first declarator, the line comment ends its line, and the glued block leads the next.
	let a1, // line
		/* block */ b1;

	// A glued block alone keeps the declarators inline.
	let a2, /* block */ b2;
</script>

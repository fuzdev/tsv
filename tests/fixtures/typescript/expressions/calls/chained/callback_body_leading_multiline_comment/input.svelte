<script>
	// A multi-line block comment before a lone callback's call body keeps the body on its last line
	a.b().c((x) =>
		/* c
		c */ a.b().c(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	);
	promise.then((x) =>
		/* c
		c */ a.b().c(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	);
	x.y((x) =>
		/* c
		c */ a.b().c(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	);
	fn((x) =>
		/* c
		c */ a.b().c(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	);

	// The same with a JSDoc-shaped comment
	a.b().c((x) =>
		/** c
		 * c */ a.b().c(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	);
	promise.then((x) =>
		/** c
		 * c */ a.b().c(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	);
	x.y((x) =>
		/** c
		 * c */ a.b().c(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	);
</script>

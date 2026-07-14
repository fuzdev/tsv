<script>
	// An authored blank line before an argument whose leading annotation is owned is
	// preserved, exactly as a same-width ordinary comment's is: the annotation's own
	// newlines are not an authored blank line, but a real blank line before it is kept.

	// Plain call
	foo(
		a,

		/* @__PURE__ */ bar()
	);
	foo(
		a,

		/* comment12 */ bar()
	);

	// new expression
	const x = new Foo(
		a,

		/* @__PURE__ */ bar()
	);

	// member-chain call
	obj.method(
		a,

		/* @__PURE__ */ bar()
	);

	// A multi-line annotation with NO authored blank: its own newlines must not read as one
	foo(
		a,
		/* @__PURE__
		   keep */ bar()
	);
</script>

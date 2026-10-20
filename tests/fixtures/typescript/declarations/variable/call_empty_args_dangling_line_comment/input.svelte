<script lang="ts">
	// Empty parens holding only a `//` are no break point of their own: the `=` breaks at any width

	// Assignment and declarator
	a =
		f(
			// c
		);
	const b =
		f(
			// c
		);

	// Member callee
	const m =
		a.b.c(
			// c
		);

	// Object property value and class field
	const o1 = {
		pppppppp:
			f(
				// c
			)
	};
	class C {
		p =
			f(
				// c
			);
	}

	// A key narrower than five columns hugs its value
	const o2 = {
		p: f(
			// c
		)
	};
	const o3 = {
		pppp: f(
			// c
		)
	};

	// A five-column key breaks after the `:`
	const o4 = {
		ppppp:
			f(
				// c
			)
	};
</script>

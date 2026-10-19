<script lang="ts">
	// A class expression is never a huggable argument: prettier's `couldExpandArg` has no
	// class arm, so a member call breaks a sole class argument onto its own line exactly as
	// a plain call does.

	// Member call, chained call, a longer chain, optional call, and a chain on a `new` head
	a.b(
		class A {
			m() {}
		}
	);
	a.b().c(
		class A {
			m() {}
		}
	);
	a.b()
		.c()
		.d(
			class A {
				m() {}
			}
		);
	a?.b(
		class A {
			m() {}
		}
	);
	new C().b(
		class A {
			m() {}
		}
	);

	// With a heritage clause, an `implements` clause, a decorator and a comment-only body,
	// and through `as` / `satisfies` / `<T>` / `!`
	a.b(
		class A extends Base {
			m() {}
		}
	);
	a.b(
		class A implements I {
			m() {}
		}
	);
	a.b(
		@d
		class A {
			m() {}
		}
	);
	a.b(
		class A {
			// c
		}
	);
	a.b(
		class A extends Base {
			m() {}
		} as any
	);
	a.b(
		class A {
			m() {}
		} satisfies T
	);
	a.b(
		<T>class A {
			m() {}
		}
	);
	a.b(
		class A {
			m() {}
		}!
	);

	// An empty body has nothing to break, so it stays on the call's line, unless a
	// decorator puts the class on lines of its own
	a.b(class {});
	a.b(class A extends Base {});
	a.b(
		@d
		class {}
	);

	// Positions: an assignment, a nested argument
	x = a.b(
		class A {
			m() {}
		}
	);
	fn(
		a.b(
			class A {
				m() {}
			}
		)
	);

	// Controls: a plain call, a class after another argument, an object and a function
	fn(
		class A {
			m() {}
		}
	);
	a.b(
		x,
		class A {
			m() {}
		}
	);
	a.b({
		m: 1
	});
	a.b(function () {
		return 1;
	});
</script>

{a.b(
	class A {
		m() {}
	}
)}
<div
	data-attr={a.b(
		class A {
			m() {}
		}
	)}
></div>

<script lang="ts">
	// A lone empty-bodied block arrow with a block comment, under a member callee: the call
	// stays on one line while it fits, a comment leading or trailing the argument
	a.fn(
		/* l */ (q) => {}
	);
	a.fn(
		(q) => {} /* c */
	);
	a.fn(
		/** l */ (q) => {}
	);
	this.fn(
		/* l */ (q) => {}
	);

	// `async` and return-typed arrows
	a.fn(
		/* l */ async (q) => {}
	);
	a.fn(
		/* l */ (q): void => {}
	);

	// An optional call and type arguments
	a?.fn(
		/* l */ (q) => {}
	);
	a.fn<T>(
		/* l */ (q) => {}
	);

	// A call that is not the chain's last keeps its argument on its line too
	a.b(
		/* l */ (q) => {}
	).c();
	a.b(
		(q) => {} /* c */
	).c();
	a.b(
		/* l */ (q) => {}
	)
		.c()
		.d();

	// An expanded member chain keeps its last call's argument on the call's line
	a.b()
		.c()
		.fn(
			/* l */ (q) => {}
		);
	a.b()
		.c()
		.fn(
			(q) => {} /* c */
		);

	// Positions: a declarator, an assignment, a return, an argument, a property value
	const v = a.fn(
		/* l */ (q) => {}
	);
	w = a.fn(
		(q) => {} /* c */
	);
	function fn1() {
		return a.fn(
			/* l */ (q) => {}
		);
	}
	fn2(
		fn3(
			a.fn(
				/* l */ (q) => {}
			)
		)
	);
	const o = {
		k: a.fn(
			/* l */ (q) => {}
		)
	};

	// Exactly 100 columns: stays on one line
	const vv = a.fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(
		/* l */ (q) => {}
	);

	// 101 columns: the argument breaks out
	const vw = a.fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(
		/* l */ (q) => {}
	);

	// A non-empty body breaks the argument out, as does a line comment
	a.fn(
		/* l */ (q) => {
			x();
		}
	);
	a.fn(
		// l
		(q) => {}
	);

	// A non-empty body in a two-call chain: the broken-out argument keeps the chain on one line —
	// the call's line with the argument's first line joined is 100 columns, then 101
	const v100 = a.b().fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(
		/* l */ (q) => {
			x();
		}
	);
	const v101 = a.b().fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(
		/* l */ (q) => {
			x();
		}
	);
	const v100t = a.b().fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(
		(q) => {
			x();
		} /* c */
	);
	const v101t = a.b().fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(
		(q) => {
			x();
		} /* c */
	);

	// A plain callee, `new`, a second argument and no comment: on one line
	fn(/* l */ (q) => {});
	new Fn(/* l */ (q) => {});
	a.fn(x, /* l */ (q) => {});
	a.fn((q) => {});
</script>

<p>{a.fn(
	/* l */ (q) => {}
)}</p>

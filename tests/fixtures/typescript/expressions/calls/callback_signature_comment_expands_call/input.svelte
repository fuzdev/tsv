<script lang="ts">
	// A preceding argument does not restore the hug: the last argument's signature is
	// forced open, so the call expands
	fn(
		x,
		(
			y // c1
		) => {
			call(y);
		}
	);

	// The `function` spelling takes the same path
	fn(
		x,
		function (
			y // c2
		) {
			call(y);
		}
	);

	// A multiline block forces it too
	fn(
		x,
		(
			y /* c3
c3 */
		) => {
			call(y);
		}
	);

	// An own-line block forces it too — it is not a glued block, and forces the list open
	fn(
		x,
		(
			y
			/* c4 */
		) => {
			call(y);
		}
	);

	// Three arguments behave the same
	fn(
		x,
		w,
		(
			y // c5
		) => {
			call(y);
		}
	);

	// A member chain's arguments take the same path
	a.b().c(
		x,
		(
			y // c6
		) => {
			call(y);
		}
	);
	a.b().c(
		x,
		function (
			y // c7
		) {
			call(y);
		}
	);

	// A `new` expression's arrow argument too
	new Comp(
		x,
		(
			y // c8
		) => {
			call(y);
		}
	);

	// The own-line kind at the hug states that already refused the other two:
	// an object body, and expression bodies single-arg, chained and under `new`
	fn(
		x,
		(
			y
			/* c9 */
		) => ({ y })
	);
	fn(
		(
			y
			/* c10 */
		) => call(y)
	);
	a.b().c(
		(
			y
			/* c11 */
		) => call(y)
	);
	new Comp(
		(
			y
			/* c12 */
		) => call(y)
	);

	// Control: a `new` expression hugs a `function` argument, where its arrow twin expands
	new Comp(x, function (
		y // c13
	) {
		call(y);
	});

	// Control: a glued single-line block forces nothing, so the hug stands
	fn(x, (y /* c14 */) => {
		call(y);
	});

	// Control: no comment
	fn(x, (y) => {
		call(y);
	});

	// The comment need not sit after the LAST parameter — anywhere in the signature that
	// forces a break refuses the hug just the same: leading a parameter, ...
	fn(
		(
			/* c15
c15 */ a
		) => {
			call(a);
		}
	);
	fn(
		(
			// c16
			a
		) => {
			call(a);
		}
	);

	// ... between parameters, ...
	fn(
		(
			a, // c17
			b
		) => {
			call(a);
		}
	);
	fn(
		(
			a,
			/* c18
c18 */ b
		) => {
			call(a);
		}
	);

	// ... and in the return-type gap
	fn(
		(
			a
		): /* c19
c19 */ T => {
			call(a);
		}
	);

	// The `function` spelling, same positions
	fn(
		function (
			/* c20
c20 */ a
		) {
			call(a);
		}
	);
	fn(
		function (
			// c21
			a
		) {
			call(a);
		}
	);

	// A member chain and a `new` expression take the same path
	a.b().c(
		(
			/* c22
c22 */ a
		) => {
			call(a);
		}
	);
	new Comp(
		(
			/* c23
c23 */ a
		) => {
			call(a);
		}
	);

	// Control: a glued single-line block leading a parameter forces nothing, so the hug stands
	fn((/* c24 */ a) => {
		call(a);
	});
	fn(function (/* c25 */ a) {
		call(a);
	});

	// Control: an OWN-LINE single-line block leading a parameter is NOT break-forcing either —
	// both formatters collapse it back onto the parameter's line and keep the hug. This is the
	// one break-forcing property a layout can manufacture, so the refusal must not ask it here
	fn((/* c26 */ a) => {
		call(a);
	});

	// ... and in the TYPE-PARAMETER list, the region ahead of the parameters — a break
	// there refuses the hug just the same, in every spelling
	fn(
		x,
		<
			// c27
			T extends A
		>(
			a: T
		) => {
			call(a);
		}
	);
	a.b().c(
		x,
		<
			// c28
			T extends A
		>(
			a: T
		) => {
			call(a);
		}
	);
	new Comp(
		x,
		<
			/* c29
c29 */ T extends A
		>(
			a: T
		) => {
			call(a);
		}
	);

	// The FIRST argument's signature refuses the expand-first hug the same way
	fn(
		<
			// c30
			T extends A
		>(
			a: T
		) => {
			call(a);
		},
		x
	);
	a.b().c(
		(
			// c31
			a
		) => {
			call(a);
		},
		x
	);
	new Comp(
		(
			// c32
			a
		) => {
			call(a);
		},
		x
	);

	// Control: a `function` argument prints its type parameters outside the signature
	// question, so a break there keeps the hug — and leaves the parameters flat
	fn(x, function <
		// c33
		T extends A
	>(a: T) {
		call(a);
	});

	// Control: in the FIRST position a `function` argument keeps the hug whatever its
	// signature does — only an arrow's signature is reprinted for that hug
	fn(function (
		// c34
		a
	) {
		call(a);
	}, x);

	// An EMPTY parameter list refuses nothing that the parameter printer owns: neither a
	// broken type-parameter list nor a comment dangling in the `()` defeats the hug
	fn(x, <
		// c35
		T extends A
	>() => {
		call();
	});
	fn(x, (
		// c36
	): T => {
		call();
	});
	fn(x, function (
		// c37
	): T {
		call();
	});

	// ... but an arrow's RETURN TYPE is asked separately, so it still refuses even there
	fn(
		x,
		() /* c38
c38 */ : T => {
			call();
		}
	);

	// A `function`'s return type is not asked at all, so a break there keeps the hug —
	// the one region where the two callback kinds answer differently
	fn(x, function (a): /* c39
c39 */ T {
		call(a);
	});
</script>

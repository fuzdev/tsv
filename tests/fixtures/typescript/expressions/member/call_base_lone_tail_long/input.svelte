<script lang="ts">
	// A lone `.prop` off a call WITH ARGUMENTS takes no break point when the chain sits
	// directly under an assignment or a declarator — prettier's `printMemberExpression`
	// call-object clause (member.js `shouldInline`). The width sheds into the call's
	// arguments instead of dropping the lookup to a line of its own.

	// a declarator initializer
	const aa1 =
		bbbbbbb(
			ccccccc
		).dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// an assignment value — the same position
	aaaaaaaaaaaa =
		bbbbbbb(
			ccccccc
		).dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// an optional lookup is the same clause — the walk steps over the `ChainExpression`
	const aa2 =
		bbbbbbb(
			ccccccc
		)?.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// NOT the clause: the call has no arguments, so the lookup keeps its break point
	const aa3 =
		bbbbbbb()
			.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// NOT the clause: two lookups, so the last one's object is a member and not the call
	const aa4 =
		bbbbbbb(ccccccc).dd
			.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// NOT the clause: a statement is neither an assignment nor a declarator
	bbbbbbb(ccccccc)
		.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// The clause names two parent types and no others. The four positions below all read
	// like one of them and are not, so the lookup keeps its break point in each.

	// a class field is a `PropertyDefinition`
	class Cc {
		aa5 =
			bbbbbbb(ccccccc)
				.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;
	}

	// an object property value
	const aa6 = {
		kk: bbbbbbb(ccccccc)
			.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
	};

	// a default parameter is an `AssignmentPattern`
	function ff(
		aa7 = bbbbbbb(ccccccc)
			.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
	) {}

	// an arrow body
	const aa8 = () =>
		bbbbbbb(ccccccc)
			.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// Control: it all fits
	const a = b(c).d;
</script>

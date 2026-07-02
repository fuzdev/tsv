<script lang="ts">
	// `using` as a plain identifier — member, index, and assignment targets,
	// not a resource declaration
	using[0] = a;
	using.b = c;
	using = d;

	// `await using` where `using` is a member expression is an await expression
	const fn1 = async () => {
		await using[0];
	};

	// for-of with `using` as the loop variable — `using of` never begins a
	// using declaration (a using binding cannot be named `of`)
	for (using of items) {
	}
	for (using of of) {
	}

	// for-in with `using` as the loop variable — a using declaration has no
	// for-in form
	for (using in b) {
	}

	// member, call, and tagged-template forms of `using` in a for-of head are
	// expressions, not declarations
	for (using.a of items) {
	}
	for (using().a of items) {
	}
	for (using``.a of items) {
	}

	// for-await-of with `using` as the loop variable
	const fn2 = async () => {
		for await (using of items) {
		}
	};

	// `await using` followed by an operator is an await expression, not an
	// `await using` declaration
	const fn3 = async () => {
		(await using) in b;
		(await using) instanceof c;
	};
</script>

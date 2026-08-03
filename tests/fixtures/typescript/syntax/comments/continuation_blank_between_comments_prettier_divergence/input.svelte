<script lang="ts">
	// declaration header: keyword->name gap
	const // c1

		// c2
		a = 1;

	// annotation `:`->type gap
	let b: // c1

		// c2
		A;

	// prefix type operator operand gap
	type C = keyof // c1

		// c2
		D;

	// callee->empty argument list gap
	fn // c1

		// c2
		();

	// before-`=` initializer gap
	const e // c1

		// c2
		= 1;

	// blank between the last comment and the tail
	const f // c1

		= 1;

	// value-side `=`->value gap, already preserved (control)
	const g =
		// c1

		// c2
		1;
</script>

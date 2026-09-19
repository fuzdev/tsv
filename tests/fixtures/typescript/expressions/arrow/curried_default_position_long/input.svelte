<script>
	// Boundary: the whole chain fits at exactly 100 — stays inline
	function fn1() {
		return (argument1) => (argument2) => (argument3) => (argument4) => argument1 + argument2 + a123;
	}

	// Boundary: one char over (101) — the heads still fit together, so only the body hangs
	function fn2() {
		return (argument1) => (argument2) => (argument3) => (argument4) =>
			argument1 + argument2 + a1234;
	}

	// Heads that do not fit together each take their own line, indented under the first,
	// and the body hangs one level under them
	function fn2b() {
		return (argument1) =>
			(argument2) =>
			(argument3) =>
			(argument4) =>
			(argument5) =>
			(argument6) =>
			(argument7) =>
				argument1 + argument2;
	}

	// A block terminal: the heads and the `{` fit at exactly 100
	function fn3() {
		return (argument1) => (argument2) => (argument3) => (argument4) => (argument5) => (a123456) => {
			return argument1;
		};
	}

	// One char over (101) — the heads stack and the block hugs the last one
	function fn4() {
		return (argument1) =>
			(argument2) =>
			(argument3) =>
			(argument4) =>
			(argument5) =>
			(a1234567) => {
				return argument1;
			};
	}

	// The same break under export default, one char over (101)
	export default (argument1) =>
		(argument2) =>
		(argument3) =>
		(argument4) =>
		(a1234567890123456) => {
			return argument1;
		};
</script>

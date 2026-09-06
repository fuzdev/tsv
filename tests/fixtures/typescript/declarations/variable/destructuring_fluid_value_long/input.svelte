<script>
	// Object pattern with an object value: `= {` lands on column 100, so the value hugs the `=`
	const { aaaaaaaaa1, aaaaaaaaa2, aaaaaaaaa3, aaaaaaaaa4, aaaaaaaaa5, aaaaaaaaa6, aaaaaaaaaaa7 } = {
		aaaaaaaaa1: 1,
		aaaaaaaaa2: 2,
		aaaaaaaaa3: 3,
		aaaaaaaaa4: 4,
		aaaaaaaaa5: 5,
		aaaaaaaaa6: 6,
		aaaaaaaaaaa7: 7
	};

	// One column more: the pattern stays flat and the value drops after the `=` (101 visual)
	const { bbbbbbbbb1, bbbbbbbbb2, bbbbbbbbb3, bbbbbbbbb4, bbbbbbbbb5, bbbbbbbbb6, bbbbbbbbbbbb7 } =
		{
			bbbbbbbbb1: 1,
			bbbbbbbbb2: 2,
			bbbbbbbbb3: 3,
			bbbbbbbbb4: 4,
			bbbbbbbbb5: 5,
			bbbbbbbbb6: 6,
			bbbbbbbbbbbb7: 7
		};

	// An array value drops the same way, and fits flat on its own line (101 visual)
	const { ccccccccc1, ccccccccc2, ccccccccc3, ccccccccc4, ccccccccc5, ccccccccc6, cccccccccccc7 } =
		[x1, x2, x3, x4, x5, x6, x7];

	// A function value is measured through `function () {`: it hugs the `=` at 100
	const { ddddddd1, ddddddd2, ddddddd3, ddddddd4, ddddddd5, ddddddd6, ddddddddddd7 } = function () {
		return ddddddd1;
	};

	// and drops after the `=` at 101
	const { eeeeeee1, eeeeeee2, eeeeeee3, eeeeeee4, eeeeeee5, eeeeeee6, eeeeeeeeeeee7 } =
		function () {
			return eeeeeee1;
		};

	// A pattern wider than the print width breaks itself, and the value hugs `} = {`
	const {
		fffffffff1,
		fffffffff2,
		fffffffff3,
		fffffffff4,
		fffffffff5,
		fffffffff6,
		fffffffffffffffffffffffffffff7
	} = {
		fffffffff1: 1,
		fffffffff2: 2,
		fffffffff3: 3,
		fffffffff4: 4,
		fffffffff5: 5,
		fffffffff6: 6,
		fffffffffffffffffffffffffffff7: 7
	};
</script>

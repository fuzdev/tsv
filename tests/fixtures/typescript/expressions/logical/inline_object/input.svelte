<script lang="ts">
	// Logical && with object literal - object hugs on same line
	const a = [
		cond1 && {
			prop1: 'value1',
			prop2: 'value2'
		},
		cond2 && {
			prop: 'value',
			fn: () => {
				a = {
					b: 'value1',
					c: 'value2'
				};
			}
		}
	];

	// Logical || with object literal
	const b = fn(
		(x ? y : z) || {
			prop1: 0,
			prop2: 'value'
		}
	);

	// Logical ?? with object literal
	const c = fn(
		expr ?? {
			prop1: 'value1',
			prop2: 'value2'
		}
	);

	// Logical && with array literal
	const d = [cond && [1, 2, 3]];

	// Chained logical with object on last operand
	const e = [
		cond1 &&
			cond2 && {
				prop: 'value'
			}
	];
</script>

<script lang="ts">
	// Declaration contexts
	{
		const as = 'a';
	}
	{
		let as = 'b';
	}

	// Destructuring
	{
		const { as } = obj;
	}
	{
		const [as] = arr;
	}
	{
		const { as: a } = obj;
	}
	{
		const { from, as } = obj;
	}

	// Assignment
	{
		let as;
		as = 'updated';
	}

	// Function parameter
	function fn1(as: string) {
		return as;
	}

	// Arrow function parameter
	const fn2 = (as: string) => as;

	// Object property shorthand
	{
		const as = 'a';
		const obj2 = { as };
	}

	// Object property key
	const obj3 = { as: 'value' };

	// Method name
	const obj4 = {
		as() {
			return 'a';
		}
	};

	// For-of with destructuring (common pattern)
	{
		for (const [from, as] of pairs) {
		}
	}
</script>

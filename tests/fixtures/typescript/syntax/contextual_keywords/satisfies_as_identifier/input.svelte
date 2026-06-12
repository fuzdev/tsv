<script lang="ts">
	// Declaration contexts
	{
		const satisfies = 'a';
	}
	{
		let satisfies = 'b';
	}

	// Destructuring
	{
		const {satisfies} = obj;
	}
	{
		const [satisfies] = arr;
	}
	{
		const {satisfies: s} = obj;
	}

	// Assignment
	{
		let satisfies;
		satisfies = 'updated';
	}

	// Function parameter
	function fn1(satisfies: string) {
		return satisfies;
	}

	// Arrow function parameter
	const fn2 = (satisfies: string) => satisfies;

	// Object property shorthand
	{
		const satisfies = 'a';
		const obj2 = {satisfies};
	}

	// Object property key
	const obj3 = {satisfies: 'value'};

	// Method name
	const obj4 = {
		satisfies() {
			return 'a';
		},
	};

	// Don't confuse with satisfies operator
	const x = {a: 1} satisfies Record<string, number>;
</script>

<script lang="ts">
	// Declaration contexts
	{
		const symbol = 'a';
	}
	{
		let symbol = 'b';
	}
	{
		var symbol = 'c';
	}

	// Destructuring
	{
		const { symbol } = obj;
	}
	{
		const [symbol] = arr;
	}
	{
		const { symbol: s } = obj;
	}

	// Assignment (Bug #5 - fails without declaration keyword)
	{
		let symbol;
		symbol = 'updated';
	}

	// Function parameter
	function fn1(symbol: string) {
		return symbol;
	}

	// Arrow function parameter
	const fn2 = (symbol: string) => symbol;

	// Object property shorthand
	{
		const symbol = 'a';
		const obj2 = { symbol };
	}

	// Object property key
	const obj3 = { symbol: 'value' };

	// Method name
	const obj4 = {
		symbol() {
			return 'a';
		}
	};

	// Class property
	class A {
		symbol = 'a';
	}

	// Type position (should remain as type keyword)
	type T = symbol;
</script>

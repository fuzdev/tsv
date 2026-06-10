<script lang="ts">
	// Function declaration
	function fn1<T> /* c */(x: T) {}

	// Async function declaration
	async function fn2<T> /* c */(x: T) {}

	// Function expression
	const a = function fn3<T> /* c */(x: T) {};

	// Class method
	class A {
		m<T> /* c */(x: T) {}
		async n<T> /* c */(x: T) {}
		*g<T> /* c */(x: T) {}
	}

	// Object method
	const obj = {m<T> /* c */(x: T) {}};
</script>

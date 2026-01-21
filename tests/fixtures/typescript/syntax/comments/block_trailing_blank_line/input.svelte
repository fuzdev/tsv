<script lang="ts">
	// Blank line before trailing comment in various block contexts

	// Function
	function fn() {
		const x = 1;

		// trailing after blank line
	}

	// Arrow function
	const arrow = () => {
		const x = 1;

		// trailing after blank line
	};

	// Class method
	class A {
		method() {
			const x = 1;

			// trailing after blank line
		}

		constructor() {
			this.x = 1;

			// trailing after blank line
		}
	}

	// If block
	if (cond) {
		a();

		// trailing after blank line
	}

	// While block
	while (cond) {
		b();

		// trailing after blank line
	}

	// For block
	for (const x of items) {
		c();

		// trailing after blank line
	}

	// Try/catch blocks
	try {
		d();

		// trailing after blank line
	} catch (e) {
		e();

		// trailing after blank line
	} finally {
		f();

		// trailing after blank line
	}
</script>

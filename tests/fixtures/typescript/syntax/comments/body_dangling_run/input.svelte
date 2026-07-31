<script lang="ts">
	// A dangling comment run - the only content of an otherwise empty body - keeps every
	// comment on its own line, including one that follows a block comment.

	// Class body - block then block.
	class A {
		/* c1 */
		/* c2 */
	}

	// Function body - block then line.
	function fn() {
		/* c1 */
		// c2
	}

	// Interface body - block then block.
	interface B {
		/* c1 */
		/* c2 */
	}

	// Namespace body - block then block.
	namespace C {
		/* c1 */
		/* c2 */
	}

	// Type literal - block then block.
	type D = {
		/* c1 */
		/* c2 */
	};

	// Enum body - block then block.
	enum E {
		/* c1 */
		/* c2 */
	}

	// Object literal - block then block.
	const obj = {
		/* c1 */
		/* c2 */
	};

	// Array literal - block then block.
	const arr = [
		/* c1 */
		/* c2 */
	];
</script>

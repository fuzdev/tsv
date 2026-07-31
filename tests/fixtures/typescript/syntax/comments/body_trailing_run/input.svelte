<script lang="ts">
	// Every comment in an end-of-body trailing run keeps its own line, including one
	// that follows a block comment.

	// Class body - block then block.
	class A {
		a = 1;
		/* c1 */
		/* c2 */
	}

	// Class body - block then line.
	class B {
		a = 1;
		/* c1 */
		// c2
	}

	// Interface body - block then block.
	interface C {
		a: string;
		/* c1 */
		/* c2 */
	}

	// Type literal body - block then block.
	type D = {
		a: string;
		/* c1 */
		/* c2 */
	};

	// Enum body - block then block.
	enum E {
		A
		/* c1 */
		/* c2 */
	}

	// Namespace body - block then block.
	namespace F {
		const a = 1;
		/* c1 */
		/* c2 */
	}

	// Function body - the same run (control).
	function fn() {
		const a = 1;
		/* c1 */
		/* c2 */
	}

	// A blank line between the comments is preserved (control).
	class G {
		a = 1;
		/* c1 */

		/* c2 */
	}
</script>

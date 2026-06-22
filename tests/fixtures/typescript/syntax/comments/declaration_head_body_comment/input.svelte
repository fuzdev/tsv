<script lang="ts">
	// Function declaration - block comment before body
	function a() /* comment */ {}

	// Function expression - block comment before body
	let b = function () /* comment */ {};

	// Method - block comment before body
	let c = {
		d() /* comment */ {}
	};

	// Class - block comment before body
	class A /* comment */ {}

	// Interface - block comment before body
	interface B /* comment */ {
		x: number;
	}

	// Enum - block comment before body
	enum C /* comment */ {
		A,
		B
	}

	// Namespace - block comment before body
	namespace D /* comment */ {
		export const x = 1;
	}
</script>

<script lang="ts">
	// a for-of binding without an initializer is valid
	for (const x of []) {
	}

	// a destructuring default inside the pattern is not a declarator initializer
	for (const { x = 1 } of []) {
	}

	// an array-element default likewise stays valid
	for (const [a = 1] of []) {
	}

	// a C-style for loop keeps its initializer
	for (let i = 0; i < 1; i++) {}
</script>

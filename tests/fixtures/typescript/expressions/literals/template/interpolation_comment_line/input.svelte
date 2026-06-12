<script lang="ts">
	// Line comment before expression in template literal
	const a = `${
		// comment
		a.b.c
	}`;

	// Multiple line comments
	const b = `${
		// comment 1
		// comment 2
		expr
	}`;

	// Line comment with complex expression
	const c = `${
		// Get the value
		obj.method().chain()
	}`;
</script>

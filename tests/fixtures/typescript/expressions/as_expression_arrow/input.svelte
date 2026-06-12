<script lang="ts">
	// Arrow function with type assertion - parens required
	// Without parens: (...args) => expr as T would be (...args) => (expr as T)
	const a = ((...args) => expr) as T;

	// Simple arrow with as - parens required
	const b = ((x) => x) as T;

	// Arrow returning type assertion - no outer parens (as applies to return value)
	const c = (x) => x as T;
</script>

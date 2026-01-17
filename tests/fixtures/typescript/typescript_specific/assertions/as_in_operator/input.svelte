<script lang="ts">
	// as expression with in operator - parentheses required
	const a = (x as string) in obj;
	const b = (key as keyof T) in obj;
	const c = ('prop' as string) in obj;
	const d = (fn() as string) in obj;
	const e = ((x + y) as string) in obj;

	// satisfies expression with in operator - parentheses required
	const f = (x satisfies string) in obj;

	// as/satisfies with instanceof - parentheses required
	const g = (x as T) instanceof Date;
	const h = (x satisfies T) instanceof Map;
</script>

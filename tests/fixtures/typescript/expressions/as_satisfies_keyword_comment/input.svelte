<script lang="ts">
	// Comment between as keyword and type
	const a = x as /* c */ string;

	// Comment between satisfies keyword and type
	const b = x satisfies /* c */ string;

	// Comment between expression and as keyword
	const c = x /* c */ as string;

	// Comment between expression and satisfies keyword
	const d = x /* c */ satisfies string;
</script>

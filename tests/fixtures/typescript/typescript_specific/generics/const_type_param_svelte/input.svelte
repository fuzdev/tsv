<script lang="ts">
	// const type parameter - Svelte context requires trailing comma for arrow functions
	// See const_type_param/input.ts for pure TypeScript version (no trailing comma)
	function literal<const T>(value: T): T {
		return value;
	}

	function tuple<const T extends readonly unknown[]>(items: T): T {
		return items;
	}

	const a = literal('');
	const b = tuple([1, 2, 3]);

	// Arrow function needs trailing comma in Svelte (JSX disambiguation)
	const arrow = <const T,>(x: T) => x;
</script>

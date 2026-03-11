<script lang="ts">
	// Object type with default: 100 chars - stays inline
	const fn1 = (a: {b?: string; c?: string; d?: string; e?: string; f?: stringABCDEFGHI} = {}) => {};

	// Object type with default: 101 chars - params expand, type stays inline
	const fn2 = (
		a: {b?: string; c?: string; d?: string; e?: string; f?: stringABCDEFGHIJ} = {},
	) => {};

	// Object type with default, type too long even inline - params expand, type expands
	const fn3 = (
		a: {
			b?: string;
			c?: string;
			d?: string;
			e?: string;
			f?: string;
			g?: string;
			h?: string;
			i?: string;
			j?: string;
			k?: string;
		} = {},
	) => {};

	// Without default, type breaks inline (no param expansion) - contrast case
	const fn4 = (a: {
		b?: string;
		c?: string;
		d?: string;
		e?: string;
		f?: string;
		g?: string;
		h?: string;
		i?: string;
		j?: string;
		k?: string;
	}) => {};

	// With return type, params expand
	const fn5 = (
		a: {b?: string; c?: string; d?: string; e?: string; f?: stringABCDEFGHIJ} = {},
	): void => {};

	// Function declaration: 100 chars - stays inline
	function fn6(a: {b?: string; c?: string; d?: string; e?: string; f?: stringABCDEFGHIJKLM} = {}) {}

	// Function declaration: 101 chars - params expand
	function fn7(
		a: {b?: string; c?: string; d?: string; e?: string; f?: stringABCDEFGHIJKLMN} = {},
	) {}
</script>

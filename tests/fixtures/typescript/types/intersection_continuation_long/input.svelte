<script lang="ts">
	// Short - stays inline
	const a: A & B & { c: C } = $props();

	// Long with type literal - wraps at & with continuation indentation
	const {
		b,
		c,
		...rest
	}: Omit<Props<typeof Comp>, 'a' | 'b'> &
		Omit<Elements['a'], 'children'> & {
			fn: () => void;
			prop1?: boolean | undefined;
			prop2?: string | undefined;
		} = $props();
</script>

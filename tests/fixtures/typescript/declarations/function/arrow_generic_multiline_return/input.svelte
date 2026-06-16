<script lang="ts">
	const fn = <T extends object>(): A<T> & {
		a: Array<T>;
		b: Array<string>;
		c: boolean;
	} => ({}) as any;
</script>

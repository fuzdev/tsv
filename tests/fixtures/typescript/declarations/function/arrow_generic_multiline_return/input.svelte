<script lang="ts">
	const fn = <T,>(): A<T> & {
		a: Array<T>;
		b: Array<string>;
		c: boolean;
	} => ({}) as any;
</script>

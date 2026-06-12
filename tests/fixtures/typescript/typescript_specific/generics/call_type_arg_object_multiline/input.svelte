<script lang="ts">
	// Single object type arg - hugs opening bracket
	const a = fn<{
		a: number;
		b: string;
	}>();

	// Method with object type arg
	const b = obj.method<{
		x: number;
		y: number;
	}>('arg');

	// With simple call arg
	const c = fn<{
		a: number;
		b: string;
	}>('arg');
</script>

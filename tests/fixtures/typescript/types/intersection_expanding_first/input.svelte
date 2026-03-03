<script lang="ts">
	let {
		prop1,
		prop2,
		...attrs
	}: {
		prop1: string;
		prop2?: Record<string, unknown>;
	} & HTMLAttributes<HTMLDivElement> = $props();

	let {
		a,
		...rest
	}: {
		a: string;
	} & OtherType = $props();
</script>

<script lang="ts">
	interface Props {
		a: string;
		b?: number;
	}

	let { a, b }: Props = $props();
</script>

<p>{a}{b}</p>

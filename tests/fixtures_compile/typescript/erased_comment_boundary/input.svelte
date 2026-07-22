<script lang="ts">
	/** A leading comment sits BEFORE the erased region and survives. */
	interface Props {
		a: string;
	}

	let { a }: Props = $props();
</script>

<p>{a}</p>

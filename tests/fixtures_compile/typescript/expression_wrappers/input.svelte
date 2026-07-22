<script lang="ts">
	let y: any = 1;
	let a = y as string;
	let b = y!;
	let c = y satisfies number;
	let d = <number>y;
</script>

<p>{a}{b}{c}{d}</p>

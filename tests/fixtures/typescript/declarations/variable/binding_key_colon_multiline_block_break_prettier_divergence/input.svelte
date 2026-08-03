<script lang="ts">
	// A multiline block the author broke after keeps the break: `: type` drops to
	// a continuation line indented one level.
	let x /* a
b */
		: string;

	// One whose type shares its closing line stays glued.
	let y /* c
d */ : number;
</script>

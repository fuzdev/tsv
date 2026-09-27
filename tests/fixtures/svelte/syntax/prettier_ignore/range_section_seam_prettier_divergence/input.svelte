<svelte:options runes />

<script module>
	export const m = 1;
</script>

<script>
	let a = 1;
</script>

<!-- format-ignore-start -->
{#if a}block1{/if} <span>inline3</span>
<!-- format-ignore-end -->
<!-- prettier-ignore-start -->
{a} text2
<!-- prettier-ignore-end -->
<!-- prettier-ignore-start -->
text1
text2
<!-- prettier-ignore-end -->
<!-- prettier-ignore-start -->
text3

text4
<!-- prettier-ignore-end -->

<style>
	p {
		color: red;
	}
</style>

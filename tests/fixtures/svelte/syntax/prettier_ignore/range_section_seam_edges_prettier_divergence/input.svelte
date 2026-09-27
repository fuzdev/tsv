<svelte:options runes />

<script module>
	export const m = 1;
</script>

<script>
	let a = 1;
</script>

<p>block1</p>
<!-- prettier-ignore-start -->
x y
<!-- prettier-ignore-end -->
text1<!-- prettier-ignore-start -->
{b}<!-- prettier-ignore-end -->
<!-- prettier-ignore-start -->
{c}
<!-- prettier-ignore-end -->
<!-- prettier-ignore-start -->
text1
text2 text3
text4
<!-- prettier-ignore-end -->
<!-- prettier-ignore-start -->
x<!-- k --> y
<!-- prettier-ignore-end -->
<p>block2</p>

<style>
	p {
		color: red;
	}
</style>

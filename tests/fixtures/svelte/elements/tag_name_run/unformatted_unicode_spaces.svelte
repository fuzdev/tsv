<!--
	An opening tag's name is a raw run ending only at a JS `\s` character, `/` or
	`>`, and the whole run must name a valid element or component. A glued symbol,
	a `{`, or U+0085 NEL — which is not in JS `\s` — stays inside the run and makes
	the name invalid (see the input_invalid_* files). The `<script>` / `<style>` /
	`<svelte:options>` dispatch reads the same run, so a glued tail cannot smuggle a
	name past it either.
-->
<div >text1</div>
<my-elem>text2</my-elem>
<Foo.Bar>text3</Foo.Bar>
<foo:bar>text4</foo:bar>
<br />

<svelte:head>
	<title>text5</title>
</svelte:head>

import * as $ from 'svelte/internal/server';
import Child from './Child.svelte';
export default function Input($$renderer, $$props) {
	let { title, extra } = $$props;
	$$renderer.push(
		`<div${$.attr('title', title)}${$.attr('data-x', `a ${$.stringify(title)} b`)}>text</div> `
	);
	Child($$renderer, $.spread_props([{ name: title }, extra]));
	$$renderer.push(`<!---->`);
}

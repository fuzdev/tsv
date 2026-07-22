import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => a * 2);
	$$renderer.push(`<div${$.attr('title', `v${$.stringify(d())}`)}></div>`);
}

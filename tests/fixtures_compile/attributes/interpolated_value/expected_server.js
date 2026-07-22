import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { prop } = $$props;
	$$renderer.push(`<p${$.attr('title', `text1 ${$.stringify(prop)} text2`)}>text</p>`);
}

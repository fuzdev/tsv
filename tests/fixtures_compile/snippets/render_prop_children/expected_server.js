import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { children } = $$props;
	$$renderer.push(`<div>`);
	children($$renderer);
	$$renderer.push(`<!----></div>`);
}

import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { p } = $$props;
		$$renderer.push(`<div></div>`);
	});
}

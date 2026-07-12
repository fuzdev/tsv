import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { prop } = $$props;
		$$renderer.push(`<p>${$.escape(prop)}</p>`);
	});
}

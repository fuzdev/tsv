import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { value = void 0 } = $$props;
		$$renderer.push(`<p>hi</p>`);
		$.bind_props($$props, { value });
	});
}

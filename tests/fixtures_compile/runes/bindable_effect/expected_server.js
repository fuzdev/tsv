import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { value = 1 } = $$props;
		$$renderer.push(`<!---->${$.escape(value)}`);
		$.bind_props($$props, { value });
	});
}

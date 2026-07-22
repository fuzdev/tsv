import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const id = $.props_id($$renderer);
		let { value = void 0 } = $$props;
		$$renderer.push(`<!---->${$.escape(value)}${$.escape(id)}`);
		$.bind_props($$props, { value });
	});
}

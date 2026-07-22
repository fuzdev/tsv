import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { a = void 0, $$slots, $$events, ...rest } = $$props;
		$$renderer.push(`<!---->${$.escape(a)}`);
		$.bind_props($$props, { a });
	});
}

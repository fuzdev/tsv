import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { value: v = void 0, open = false } = $$props;
		$$renderer.push(`<!---->${$.escape(v)}${$.escape(open)}`);
		$.bind_props($$props, { value: v, open });
	});
}

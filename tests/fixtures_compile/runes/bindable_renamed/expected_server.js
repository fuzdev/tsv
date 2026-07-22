import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { value: v = void 0 } = $$props;
		$$renderer.push(`<!---->${$.escape(v)}`);
		$.bind_props($$props, { value: v });
	});
}

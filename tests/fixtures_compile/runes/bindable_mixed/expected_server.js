import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { a, b = 1, c, d = void 0 } = $$props;
		$$renderer.push(`<!---->${$.escape(a)}${$.escape(b)}${$.escape(c)}${$.escape(d)}`);
		$.bind_props($$props, { b, d });
	});
}

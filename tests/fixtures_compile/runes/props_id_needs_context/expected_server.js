import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const id = $.props_id($$renderer);
		const d = new Date();
		$$renderer.push(`<!---->${$.escape(id)}${$.escape(d.getTime())}`);
	});
}

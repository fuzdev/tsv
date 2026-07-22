import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class Store {
			data = { x: 1 };
		}
		const s = new Store();
		$$renderer.push(`<p>${$.escape(s.data.x)}</p>`);
	});
}

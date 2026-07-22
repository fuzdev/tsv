import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class Counter {
			#count = 0;
			get count() {
				return this.#count;
			}
		}
		const c = new Counter();
		$$renderer.push(`<p>${$.escape(c.count)}</p>`);
	});
}

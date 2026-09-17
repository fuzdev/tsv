import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class Counter {
			count = /* initial */ 0;
			label = /** @type {string | null} */ (null);
			pending; /* none yet */
		}
		const counter = new Counter();
		$$renderer.push(
			`<p>${$.escape(counter.count)}${$.escape(counter.label)}${$.escape(counter.pending)}</p>`
		);
	});
}

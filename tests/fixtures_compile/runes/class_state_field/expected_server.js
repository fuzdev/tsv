import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class Counter {
			count = 0;
			increment() {
				this.count += 1;
			}
		}
		const c = new Counter();
		$$renderer.push(`<button>${$.escape(c.count)}</button>`);
	});
}

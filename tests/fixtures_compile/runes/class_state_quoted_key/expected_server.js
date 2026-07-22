import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class Toggle {
			'aria-pressed' = false;
		}
		const t = new Toggle();
		$$renderer.push(`<button${$.attr('aria-pressed', t['aria-pressed'])}>x</button>`);
	});
}

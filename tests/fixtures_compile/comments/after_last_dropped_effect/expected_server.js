import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let x = 1;
		$$renderer.push(`<p>1</p>`);
		// before effect
	});
}

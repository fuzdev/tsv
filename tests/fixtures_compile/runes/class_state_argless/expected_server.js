import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class Box {
			value;
		}
		const b = new Box();
		$$renderer.push(`<p>${$.escape(b.value)}</p>`);
	});
}

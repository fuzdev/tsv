import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let a = 1;
		let b = $.derived(() => a * 2);
		// forces the wrapper
		let d = new Date();
		$$renderer.push(`<p>2${$.escape(d)}</p>`);
	});
}

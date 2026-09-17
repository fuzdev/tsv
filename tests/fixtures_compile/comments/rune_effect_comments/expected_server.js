import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let n = 1;
		// log on change
		/* debug */
		let label = 'count';
		function inc() {
			n += 1;
		}
		$$renderer.push(`<button>count ${$.escape(n)}</button>`);
	});
}

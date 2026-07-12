import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	// leading note
	let { prop } = $$props;
	let a = 1; // trailing note
	/* block note */
	let b = 2;
	$$renderer.push(`<p>${$.escape(prop)}</p>`);
}

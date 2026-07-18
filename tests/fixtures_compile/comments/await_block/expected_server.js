import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// the promise
	let p = Promise.resolve(1);
	$.await(
		$$renderer,
		p,
		() => {},
		(value) => {
			$$renderer.push(`<p>${$.escape(value)}</p>`);
		}
	);
	$$renderer.push(`<!--]-->`);
}

import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let p = Promise.resolve(1);
	$.await(
		$$renderer,
		p,
		() => {
			$$renderer.push(`<p>l</p>`);
		},
		(v) => {
			$$renderer.push(`<p>${$.escape(v)}</p>`);
		}
	);
	$$renderer.push(`<!--]-->`);
}

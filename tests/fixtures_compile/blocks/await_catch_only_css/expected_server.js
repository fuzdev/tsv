import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let promise = Promise.resolve(1);
	$.await(
		$$renderer,
		promise,
		() => {
			$$renderer.push(`<p>loading</p>`);
		},
		(v) => {
			$$renderer.push(`<p>${$.escape(v)}</p>`);
		}
	);
	$$renderer.push(`<!--]-->`);
}

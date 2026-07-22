import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { promise } = $$props;
	$.await(
		$$renderer,
		promise,
		() => {
			$$renderer.push(`<p>loading</p>`);
		},
		(value) => {
			$$renderer.push(`<p>${$.escape(value)}</p>`);
		}
	);
	$$renderer.push(`<!--]-->`);
}

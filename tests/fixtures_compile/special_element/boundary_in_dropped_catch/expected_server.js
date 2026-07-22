import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.await(
		$$renderer,
		p,
		() => {
			$$renderer.push(`<i>w</i>`);
		},
		(v) => {
			$$renderer.push(`<b>${$.escape(v)}</b>`);
		}
	);
	$$renderer.push(`<!--]-->`);
}

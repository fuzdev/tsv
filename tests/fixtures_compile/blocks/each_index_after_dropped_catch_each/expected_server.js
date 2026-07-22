import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.await(
		$$renderer,
		p,
		() => {
			$$renderer.push(`<b>x</b>`);
		},
		() => {}
	);
	$$renderer.push(`<!--]--><!--[-->`);
	const each_array = $.ensure_array_like([2]);
	for (let $$index_1 = 0, $$length = each_array.length; $$index_1 < $$length; $$index_1++) {
		let b = each_array[$$index_1];
		$$renderer.push(`<u>${$.escape(b)}</u>`);
	}
	$$renderer.push(`<!--]-->`);
}

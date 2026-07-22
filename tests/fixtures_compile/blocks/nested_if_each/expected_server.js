import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { items } = $$props;
	$$renderer.push(`<!--[-->`);
	const each_array = $.ensure_array_like(items);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let item = each_array[$$index];
		if (item) {
			$$renderer.push('<!--[0-->');
			$$renderer.push(`<li>${$.escape(item)}</li>`);
		} else {
			$$renderer.push('<!--[-1-->');
		}
		$$renderer.push(`<!--]-->`);
	}
	$$renderer.push(`<!--]-->`);
}

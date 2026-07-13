import * as $ from 'svelte/internal/server';
function row($$renderer, item) {
	$$renderer.push(`<li>${$.escape(item)}</li>`);
}
export default function Input($$renderer, $$props) {
	let { items } = $$props;
	$$renderer.push(`<ul><!--[-->`);
	const each_array = $.ensure_array_like(items);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let item = each_array[$$index];
		row($$renderer, item);
	}
	$$renderer.push(`<!--]--></ul>`);
}

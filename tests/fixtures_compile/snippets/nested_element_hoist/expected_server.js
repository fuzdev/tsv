import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { rows } = $$props;
	function cell($$renderer, value) {
		$$renderer.push(`<td>${$.escape(value)}</td>`);
	}
	$$renderer.push(`<div><!--[-->`);
	const each_array = $.ensure_array_like(rows);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let row = each_array[$$index];
		cell($$renderer, row);
	}
	$$renderer.push(`<!--]--></div>`);
}

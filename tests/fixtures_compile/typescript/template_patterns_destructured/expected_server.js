import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { rows, pairs, p } = $$props;
	$$renderer.push(`<!--[-->`);
	const each_array = $.ensure_array_like(rows);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let { a, b } = each_array[$$index];
		$$renderer.push(`<li>${$.escape(a)} ${$.escape(b)}</li>`);
	}
	$$renderer.push(`<!--]--> <!--[-->`);
	const each_array_1 = $.ensure_array_like(pairs);
	for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
		let [n, s] = each_array_1[$$index_1];
		$$renderer.push(`<li>${$.escape(n)} ${$.escape(s)}</li>`);
	}
	$$renderer.push(`<!--]--> `);
	$.await(
		$$renderer,
		p,
		() => {},
		({ v }) => {
			$$renderer.push(`<p>${$.escape(v)}</p>`);
		}
	);
	$$renderer.push(`<!--]-->`);
}

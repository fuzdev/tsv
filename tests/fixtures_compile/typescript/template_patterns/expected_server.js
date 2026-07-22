import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { items, p, n } = $$props;
	$$renderer.push(`<!--[-->`);
	const each_array = $.ensure_array_like(items);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let item = each_array[$$index];
		$$renderer.push(`<li>${$.escape(item)}</li>`);
	}
	$$renderer.push(`<!--]--> `);
	$.await(
		$$renderer,
		p,
		() => {},
		(value) => {
			$$renderer.push(`<p>${$.escape(value)}</p>`);
		}
	);
	$$renderer.push(`<!--]--> `);
	if (n) {
		$$renderer.push('<!--[0-->');
		const doubled = n * 2;
		$$renderer.push(`<p>${$.escape(doubled)}</p>`);
	} else {
		$$renderer.push('<!--[-1-->');
	}
	$$renderer.push(`<!--]-->`);
}

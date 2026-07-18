import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => ({ items: [] }));
	$$renderer.push(`<!--[-->`);
	const each_array = $.ensure_array_like(d().items);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let x = each_array[$$index];
		$$renderer.push(`<!---->${$.escape(x)}`);
	}
	$$renderer.push(`<!--]-->`);
}

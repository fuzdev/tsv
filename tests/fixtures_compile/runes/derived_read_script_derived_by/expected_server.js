import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => a * 2);
	let e = $.derived(d());
	$$renderer.push(`<button>${$.escape(a)}</button>`);
}

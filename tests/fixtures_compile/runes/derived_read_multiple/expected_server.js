import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d1 = $.derived(() => a * 2);
	let d2 = $.derived(() => a * 3);
	$$renderer.push(`<!---->${$.escape(d1() + d2())}`);
}

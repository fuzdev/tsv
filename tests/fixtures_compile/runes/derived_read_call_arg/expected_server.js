import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => a * 2);
	function f(x) {
		return x;
	}
	$$renderer.push(`<!---->${$.escape(f(d()))}`);
}

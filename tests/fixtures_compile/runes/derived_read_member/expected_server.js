import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => ({ x: a }));
	$$renderer.push(`<!---->${$.escape(d().x)}`);
}

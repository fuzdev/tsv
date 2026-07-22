import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { obj } = $$props;
	let d = $.derived(() => obj);
	$$renderer.push(`<div${$.attributes({ ...d() })}></div>`);
}

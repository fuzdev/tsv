import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	$$renderer.push(`<div${$.attributes({ ...props }, 'svelte-tsvhash')}>x</div>`);
}
